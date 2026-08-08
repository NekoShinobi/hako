use actix_cors::Cors;
use actix_files as fs;
use actix_multipart::Multipart;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Result, middleware, middleware::Compress, web,
};
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use chrono::{Duration, Utc};
use futures::StreamExt;
use hmac::{Hmac, Mac};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use sea_orm_migration::prelude::*;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::sync::LazyLock;
use tempfile::NamedTempFile;
use uuid::Uuid;

mod entities;
mod migration;

use entities::{file_access_logs, files, users};
use migration::Migrator;

type HmacSha256 = Hmac<Sha256>;

// Environment variable configuration
static DATABASE_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:/data/hako.db".to_string())
});

static UPLOAD_DIR: LazyLock<String> =
    LazyLock::new(|| std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "/uploads".to_string()));

// AES-256 takes a 32-byte key, and ENCRYPTION_KEY is consumed as raw bytes, so
// anything shorter would be NUL-padded. Enforced at startup.
const ENCRYPTION_KEY_LEN: usize = 32;

// Size of the public download token. 128 bits of CSPRNG output is far past
// guessing range, and renders as a 32-character hex path segment.
const TOKEN_BYTES: usize = 16;

// Placeholder key so a fresh checkout starts without configuration. Exactly 32
// bytes, and published in this repository — it is worth nothing as a secret.
// Every start logs a warning and the UI shows a banner until it is replaced.
const DEFAULT_ENCRYPTION_KEY: &str = "hako-insecure-default-key-change";

static ENCRYPTION_KEY: LazyLock<String> = LazyLock::new(|| {
    std::env::var("ENCRYPTION_KEY")
        .ok()
        // Empty is the `${ENCRYPTION_KEY:-}` case, not a deliberate key. Left
        // as-is it would derive an all-NUL key, which is worse than the default.
        .filter(|key| !key.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_ENCRYPTION_KEY.to_string())
});

// Whether this instance is running on the published placeholder key.
static USING_DEFAULT_KEY: LazyLock<bool> =
    LazyLock::new(|| *ENCRYPTION_KEY == DEFAULT_ENCRYPTION_KEY);

static HOST: LazyLock<String> =
    LazyLock::new(|| std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()));

static PORT: LazyLock<String> =
    LazyLock::new(|| std::env::var("PORT").unwrap_or_else(|_| "9090".to_string()));

static MAX_FILE_SIZE_MB: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("MAX_FILE_SIZE_MB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100)
});

static FILE_EXPIRY_MINUTES: LazyLock<i64> = LazyLock::new(|| {
    std::env::var("FILE_EXPIRY_MINUTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(180)
});

static CLEANUP_INTERVAL_SECS: LazyLock<u64> = LazyLock::new(|| {
    std::env::var("CLEANUP_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300)
});

static ACCESS_LOG_RETENTION_DAYS: LazyLock<i64> = LazyLock::new(|| {
    std::env::var("ACCESS_LOG_RETENTION_DAYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30)
});

static DB_MAX_CONNECTIONS: LazyLock<u32> = LazyLock::new(|| {
    std::env::var("DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10)
});

static ACTIX_WORKERS: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("ACTIX_WORKERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(num_cpus::get)
});

static HAKO_BASE_URL: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var("HAKO_BASE_URL").ok());

// Key for hashing IP addresses, derived from ENCRYPTION_KEY with a domain
// separator so the same secret does not serve directly as both the AES-GCM key
// and the MAC key. Rotating ENCRYPTION_KEY makes new hashes unlinkable to old.
static IP_HASH_KEY: LazyLock<[u8; 32]> = LazyLock::new(|| {
    let mut mac = <HmacSha256 as KeyInit>::new_from_slice(ENCRYPTION_KEY.as_bytes())
        .expect("HMAC accepts a key of any length");
    mac.update(b"hako-ip-hash-v1");
    let mut key = [0u8; 32];
    key.copy_from_slice(&mac.finalize().into_bytes());
    key
});

static IMAGE_DIR: LazyLock<String> = LazyLock::new(|| {
    std::env::var("IMAGE_DIR").unwrap_or_else(|_| "/app/static/images".to_string())
});

// Inert formats: the browser decodes them, it does not execute them. Used when
// INLINE_ALLOW_TYPES is unset.
const DEFAULT_INLINE_ALLOW: &[&str] = &[
    "image/apng",
    "image/avif",
    "image/bmp",
    "image/gif",
    "image/jpeg",
    "image/png",
    "image/webp",
    "audio/aac",
    "audio/flac",
    "audio/mpeg",
    "audio/ogg",
    "audio/wav",
    "video/mp4",
    "video/ogg",
    "video/webm",
];

// Types a browser will run script from when it renders them directly. Not a
// blocklist — only the subject of a startup warning if the operator allows one.
const SCRIPT_CAPABLE_TYPES: &[&str] = &[
    "image/svg+xml",
    "text/html",
    "application/xhtml+xml",
    "text/xml",
    "application/xml",
    "text/xsl",
    "application/xslt+xml",
];

// Types served inline as their declared type. Unset uses DEFAULT_INLINE_ALLOW;
// set (including empty) replaces it outright, so the effective policy is always
// exactly what the operator wrote.
static INLINE_ALLOW_TYPES: LazyLock<Vec<String>> = LazyLock::new(|| {
    std::env::var("INLINE_ALLOW_TYPES").map_or_else(
        |_| {
            DEFAULT_INLINE_ALLOW
                .iter()
                .copied()
                .map(str::to_string)
                .collect()
        },
        |raw| parse_type_list(&raw),
    )
});

// Types always forced to download. Checked before the allow list, so this also
// subtracts from the defaults without having to restate them.
static INLINE_DENY_TYPES: LazyLock<Vec<String>> = LazyLock::new(|| {
    std::env::var("INLINE_DENY_TYPES").map_or_else(|_| Vec::new(), |raw| parse_type_list(&raw))
});

static MAX_FILE_SIZE: LazyLock<usize> = LazyLock::new(|| *MAX_FILE_SIZE_MB * 1024 * 1024);

// Application state
struct AppState {
    db: DatabaseConnection,
    upload_dir: PathBuf,
    cipher: Aes256Gcm,
    max_file_size: usize,
    file_expiry_minutes: i64,
}

// Encryption/Decryption utilities
fn encrypt_data(cipher: &Aes256Gcm, data: &[u8]) -> Result<Vec<u8>, String> {
    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, data)
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Prepend nonce to ciphertext for storage
    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

fn decrypt_data(cipher: &Aes256Gcm, encrypted_data: &[u8]) -> Result<Vec<u8>, String> {
    if encrypted_data.len() < 12 {
        return Err("Invalid encrypted data".to_string());
    }

    let nonce =
        Nonce::try_from(&encrypted_data[..12]).map_err(|e| format!("Invalid nonce: {}", e))?;
    let ciphertext = &encrypted_data[12..];

    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))
}

// Helper: Encrypt string for database storage
fn encrypt_string(cipher: &Aes256Gcm, data: &str) -> Result<String, String> {
    let encrypted = encrypt_data(cipher, data.as_bytes())?;
    Ok(hex::encode(encrypted))
}

// Helper: Decrypt string from database
fn decrypt_string(cipher: &Aes256Gcm, encrypted_hex: &str) -> Result<String, String> {
    let encrypted =
        hex::decode(encrypted_hex).map_err(|e| format!("Failed to decode hex: {}", e))?;
    let decrypted = decrypt_data(cipher, &encrypted)?;
    String::from_utf8(decrypted).map_err(|e| format!("Failed to convert to string: {}", e))
}

// Helper: Hash IP address for storage and audit logging (one-way hash for privacy)
// Keyed rather than a bare digest: IPv4 is only 2^32 addresses, so an unkeyed
// hash of an IP can be reversed by enumerating the whole space.
fn hash_ip(ip: &str) -> String {
    let mut mac = <HmacSha256 as KeyInit>::new_from_slice(IP_HASH_KEY.as_slice())
        .expect("HMAC accepts a key of any length");
    mac.update(ip.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

// Helper: Get or create anonymous user
// Takes the raw IP and hashes it here, so plaintext addresses never reach storage.
async fn get_or_create_anonymous_user(
    db: &DatabaseConnection,
    ip_address: &str,
) -> Result<users::Model, sea_orm::DbErr> {
    let session_token = Uuid::new_v4().to_string();

    let user = users::ActiveModel {
        ip_hash: Set(hash_ip(ip_address)),
        session_token: Set(session_token),
        is_anonymous: Set(true),
        created_at: Set(Utc::now()),
        ..Default::default()
    };

    user.insert(db).await
}

// Helper: Record an internal failure in full, and hand the client nothing but a
// reference to it.
//
// Error text from the database, the filesystem and the cipher describes internal
// state — schema, paths, key handling — so none of it crosses the wire. The
// error_id is the join: a user can quote it in a bug report and an operator can
// grep the log for the real cause.
fn internal_error(context: &str, detail: impl std::fmt::Display) -> actix_web::Error {
    let error_id = hex::encode(rand::random::<[u8; 8]>());
    log::error!(
        "internal error [{}] while {}: {}",
        error_id,
        context,
        detail
    );

    actix_web::error::InternalError::from_response(
        // Kept server-side: InternalError renders the response below, not this.
        "internal error",
        HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Internal server error",
            "error_id": error_id,
        })),
    )
    .into()
}

// Helper: Log file access
async fn log_file_access(
    db: &DatabaseConnection,
    file_id: i64,
    accessed_by: &str,
) -> Result<(), sea_orm::DbErr> {
    let log = file_access_logs::ActiveModel {
        file_id: Set(file_id),
        accessed_by: Set(accessed_by.to_string()),
        access_time: Set(Utc::now()),
        ..Default::default()
    };

    log.insert(db).await?;
    Ok(())
}

// POST /upload - Handle file upload
async fn upload_file(
    mut payload: Multipart,
    data: web::Data<AppState>,
    req: HttpRequest,
) -> Result<HttpResponse> {
    let ip_address = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();

    // Audit log: Upload attempt
    log::info!("Upload attempt from IP hash: {}", hash_ip(&ip_address));

    // Create anonymous user
    let user = get_or_create_anonymous_user(&data.db, &ip_address)
        .await
        .map_err(|e| internal_error("creating an anonymous user", e))?;

    // Stage the body on disk beside its final home rather than in memory, so an
    // oversized upload costs disk we can bound instead of RAM we cannot.
    std::fs::create_dir_all(&data.upload_dir)
        .map_err(|e| internal_error("creating the upload directory", e))?;
    let mut temp_file = NamedTempFile::new_in(&data.upload_dir)
        .map_err(|e| internal_error("creating a staging file", e))?;

    let mut hasher = Sha256::new();
    let mut total: usize = 0;
    let mut file_name = String::from("unnamed");
    let mut content_type = String::from("application/octet-stream");

    // Process multipart data
    while let Some(item) = payload.next().await {
        let mut field = item.map_err(actix_web::error::ErrorBadRequest)?;

        if let Some(disposition) = field.content_disposition()
            && let Some(name) = disposition.get_filename()
        {
            // Sanitize filename to prevent directory traversal
            file_name = sanitize_filename::sanitize(name);
        }

        if let Some(ct) = field.content_type() {
            content_type = ct.to_string();
        }

        // Stream to disk, hashing as we go. Nothing but the current chunk is
        // held in memory.
        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_err(actix_web::error::ErrorBadRequest)?;

            // Enforce the limit here, while the body is arriving. Checking it
            // after the loop means buffering the whole request first, which is
            // the thing that lets an unauthenticated client exhaust memory.
            total = total.saturating_add(chunk.len());
            if total > data.max_file_size {
                log::warn!(
                    "Upload failed: exceeded {} byte limit from IP hash {}",
                    data.max_file_size,
                    hash_ip(&ip_address)
                );
                return Ok(HttpResponse::PayloadTooLarge().json(serde_json::json!({
                    "error": format!("File size exceeds maximum allowed size of {} MB", data.max_file_size / (1024 * 1024))
                })));
            }

            hasher.update(&chunk);
            temp_file
                .write_all(&chunk)
                .map_err(|e| internal_error("writing the staged upload", e))?;
        }
    }

    if total == 0 {
        log::warn!(
            "Upload failed: No file data from IP hash {}",
            hash_ip(&ip_address)
        );
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "No file data received"
        })));
    }

    temp_file
        .flush()
        .map_err(|e| internal_error("flushing the staged upload", e))?;

    // The content hash is returned to the uploader so they can verify what we
    // stored, but it is deliberately NOT the URL: it is derived from the file's
    // own bytes, so anyone holding a copy could reconstruct the link and confirm
    // the upload happened. The public identifier is unguessable instead.
    let hash = hex::encode(hasher.finalize());
    let token = hex::encode(rand::random::<[u8; TOKEN_BYTES]>());
    let file_path = data.upload_dir.join(&token);

    // Encrypting means holding the plaintext and its ciphertext at once, but
    // both are now bounded by the limit checked above. Run it on the blocking
    // pool: at MAX_FILE_SIZE_MB this is far too much disk and CPU for an async
    // worker thread.
    let cipher = data.cipher.clone();
    let staged = temp_file.path().to_path_buf();
    let target = file_path.clone();
    web::block(move || -> std::io::Result<()> {
        let plaintext = std::fs::read(&staged)?;
        let encrypted = encrypt_data(&cipher, &plaintext).map_err(std::io::Error::other)?;
        std::fs::write(&target, encrypted)
    })
    .await
    .map_err(|e| internal_error("scheduling upload encryption", e))?
    .map_err(|e| internal_error("encrypting the upload", e))?;

    // Close temp file (it will be auto-deleted)
    drop(temp_file);

    // Store file metadata in database with encryption
    let file_size = total as i64;
    let file_path_str = file_path.to_string_lossy().to_string();

    // Encrypt sensitive fields for database storage
    let encrypted_file_name = encrypt_string(&data.cipher, &file_name)
        .map_err(|e| internal_error("encrypting the file name", e))?;
    let encrypted_file_path = encrypt_string(&data.cipher, &file_path_str)
        .map_err(|e| internal_error("encrypting the file path", e))?;

    let file_model = files::ActiveModel {
        user_id: Set(Some(user.user_id)),
        file_name: Set(encrypted_file_name),
        file_path: Set(encrypted_file_path),
        token: Set(token.clone()),
        size: Set(file_size),
        mime_type: Set(Some(content_type.clone())),
        created_at: Set(Utc::now()),
        expires_at: Set(Some(
            Utc::now() + Duration::minutes(data.file_expiry_minutes),
        )),
        ..Default::default()
    };

    let inserted_file = file_model
        .insert(&data.db)
        .await
        .map_err(|e| internal_error("storing file metadata", e))?;

    // Audit log: Successful upload. The token is a capability — logging it would
    // put working download links in the log file, so record the id instead.
    log::info!(
        "File uploaded successfully: file_id={}, size={}, IP hash={}",
        inserted_file.file_id,
        file_size,
        hash_ip(&ip_address)
    );

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "file_id": inserted_file.file_id,
        "token": token,
        "hash": hash,
        "url": format!("/file/{}", token),
        "file_name": file_name,
        "size": file_size,
        "mime_type": content_type
    })))
}

// Helper: Parse a comma-separated MIME type list from configuration.
fn parse_type_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

// Helper: Match a MIME type against one configured pattern. Accepts an exact
// type, a `type/*` wildcard, or `*` for everything.
fn matches_pattern(pattern: &str, essence: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    match pattern.strip_suffix("/*") {
        Some(top) => essence.split_once('/').is_some_and(|(t, _)| t == top),
        None => pattern == essence,
    }
}

fn matches_any(patterns: &[String], essence: &str) -> bool {
    patterns.iter().any(|p| matches_pattern(p, essence))
}

// Helper: Reject anything that is not a well-formed `type/subtype` of RFC 2045
// token characters. A wildcard rule can match a subtype we have never seen, and
// the result is echoed into a response header, so validate before trusting it.
fn is_wellformed_essence(essence: &str) -> bool {
    fn is_token(c: char) -> bool {
        c.is_ascii_alphanumeric() || "!#$&-^_.+".contains(c)
    }
    essence.split_once('/').is_some_and(|(top, sub)| {
        !top.is_empty() && !sub.is_empty() && top.chars().all(is_token) && sub.chars().all(is_token)
    })
}

// Helper: Decide how a stored file may be handed back to a browser.
//
// The uploader chooses `mime_type`, so this is deny-by-default: the response
// type is one we picked, never one we were given. Returns the content type to
// send and the Content-Disposition mode.
//
// Precedence is deny, then allow, then the text fallback, then download.
// Note that no default allows a script-capable type inline — `image/svg+xml` in
// particular is a document format that runs <script> when navigated to, so
// treating it as an image would reopen the hole this closes. An operator can
// opt into it via INLINE_ALLOW_TYPES and is warned at startup.
fn presentation(mime_type: Option<&str>) -> (String, &'static str) {
    const DOWNLOAD: &str = "application/octet-stream";
    let download = || (DOWNLOAD.to_string(), "attachment");

    let Some(raw) = mime_type else {
        return download();
    };

    // Compare on the essence: the stored value may carry parameters.
    let essence = raw
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    if !is_wellformed_essence(&essence) || matches_any(&INLINE_DENY_TYPES, &essence) {
        return download();
    }

    if matches_any(&INLINE_ALLOW_TYPES, &essence) {
        return (essence, "inline");
    }

    // Text is worth previewing, but only ever as a type nothing executes.
    // This is what neutralises an uploaded text/html.
    if essence.starts_with("text/") {
        return ("text/plain; charset=utf-8".to_string(), "inline");
    }

    download()
}

// Helper: Report the effective inline policy, and warn if it renders a
// script-capable type. Called once at startup so an operator who opts in sees
// it in the log rather than discovering it from a report like this one.
fn log_inline_policy() {
    let describe = |list: &[String]| {
        if list.is_empty() {
            "(none)".to_string()
        } else {
            list.join(", ")
        }
    };
    log::info!("Inline allow list: {}", describe(&INLINE_ALLOW_TYPES));
    log::info!("Inline deny list: {}", describe(&INLINE_DENY_TYPES));

    let risky: Vec<&str> = SCRIPT_CAPABLE_TYPES
        .iter()
        .copied()
        .filter(|t| !matches_any(&INLINE_DENY_TYPES, t) && matches_any(&INLINE_ALLOW_TYPES, t))
        .collect();

    if !risky.is_empty() {
        log::warn!(
            "Inline policy renders script-capable types: {}. Uploaded files of \
             these types are rendered as documents; only the Content-Security-Policy \
             sandbox header keeps their scripts off this origin. Serve downloads from \
             a separate domain if you rely on this.",
            risky.join(", ")
        );
    }
}

// Helper: Accept only a well-formed token. New tokens are TOKEN_BYTES of hex;
// rows migrated from the old content-hash scheme carry a 64-character SHA-256.
// Rejecting anything else avoids a pointless query, and stops a crafted path
// from reaching the log — actix percent-decodes path segments, so an unchecked
// value can inject newlines and forge log entries.
fn is_valid_token(token: &str) -> bool {
    matches!(token.len(), n if n == TOKEN_BYTES * 2 || n == 64)
        && token.bytes().all(|b| b.is_ascii_hexdigit())
}

// GET /file/{token} - Retrieve file by its unguessable token
async fn get_file(
    token: web::Path<String>,
    data: web::Data<AppState>,
    req: HttpRequest,
) -> Result<HttpResponse> {
    let token = token.into_inner();

    let ip_address = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();

    // The token is a capability: anyone holding it can fetch the file, so it is
    // never written to the log. Outcomes are recorded against the file id.
    if !is_valid_token(&token) {
        log::warn!(
            "Download rejected: malformed token from IP hash {}",
            hash_ip(&ip_address)
        );
        return Err(actix_web::error::ErrorNotFound("File not found"));
    }

    // Fetch file metadata from database
    let file = files::Entity::find()
        .filter(files::Column::Token.eq(&token))
        .one(&data.db)
        .await
        .map_err(|e| internal_error("looking up a file", e))?
        .ok_or_else(|| {
            log::warn!(
                "Download failed: no file for token from IP hash {}",
                hash_ip(&ip_address)
            );
            actix_web::error::ErrorNotFound("File not found")
        })?;

    // Check if file has expired
    if let Some(expires_at) = file.expires_at
        && Utc::now() > expires_at
    {
        log::warn!(
            "Download failed: File expired: file_id={} from IP hash {}",
            file.file_id,
            hash_ip(&ip_address)
        );
        return Ok(HttpResponse::Gone().json(serde_json::json!({
            "error": "File has expired"
        })));
    }

    // Decrypt database fields
    let decrypted_file_name = decrypt_string(&data.cipher, &file.file_name)
        .map_err(|e| internal_error("decrypting the file name", e))?;
    let decrypted_file_path = decrypt_string(&data.cipher, &file.file_path)
        .map_err(|e| internal_error("decrypting the file path", e))?;

    // Read encrypted file from disk
    let encrypted_data = std::fs::read(&decrypted_file_path)
        .map_err(|e| internal_error("reading a stored file", e))?;

    // Decrypt file data
    let decrypted_data = decrypt_data(&data.cipher, &encrypted_data)
        .map_err(|e| internal_error("decrypting a stored file", e))?;

    // Log access with hashed IP
    let _ = log_file_access(&data.db, file.file_id, &hash_ip(&ip_address)).await;

    // Audit log: Successful download
    log::info!(
        "File downloaded successfully: file_id={}, size={}, IP hash={}",
        file.file_id,
        file.size,
        hash_ip(&ip_address)
    );

    // Return file with a content type we are willing to hand a browser
    let (content_type, disposition) = presentation(file.mime_type.as_deref());

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .insert_header((
            "Content-Disposition",
            format!("{}; filename=\"{}\"", disposition, decrypted_file_name),
        ))
        // Without nosniff a browser may sniff past the type we just chose.
        .insert_header(("X-Content-Type-Options", "nosniff"))
        // Opaque origin: even if something does execute, it is not our origin.
        .insert_header(("Content-Security-Policy", "sandbox"))
        .body(decrypted_data))
}

// GET /config - Return configuration information
async fn get_config(data: web::Data<AppState>) -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "file_expiry_minutes": data.file_expiry_minutes,
        "max_file_size_mb": data.max_file_size / (1024 * 1024),
        // Drives the UI banner. A boolean only — never the key itself.
        "using_default_key": *USING_DEFAULT_KEY,
    })))
}

// GET /img/{filename} - Serve static images (AVIF format)
async fn serve_image(filename: web::Path<String>) -> Result<HttpResponse> {
    let filename = filename.into_inner();

    // Sanitize filename to prevent directory traversal
    let safe_filename = sanitize_filename::sanitize(&filename);

    // Build path to image file (all images are .avif)/
    let image_path = PathBuf::from(IMAGE_DIR.as_str()).join(format!("{}.avif", safe_filename));

    // Check if file exists
    if !image_path.exists() {
        return Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Image not found"
        })));
    }

    // Read image file
    let image_data =
        std::fs::read(&image_path).map_err(|e| internal_error("reading an image", e))?;

    // Return image with AVIF content type
    Ok(HttpResponse::Ok()
        .content_type("image/avif")
        .insert_header(("Cache-Control", "public, max-age=31536000")) // Cache for 1 year
        .body(image_data))
}

// Initialize database
async fn init_db(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    Migrator::up(db, None).await
}

// Background task to clean up expired files
async fn cleanup_expired_files(
    db: DatabaseConnection,
    cipher: Aes256Gcm,
    interval_secs: u64,
    access_log_retention_days: i64,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

    loop {
        interval.tick().await;

        // Find all expired files
        let now = Utc::now();
        let expired_files = files::Entity::find()
            .filter(files::Column::ExpiresAt.is_not_null())
            .filter(files::Column::ExpiresAt.lte(now))
            .all(&db)
            .await;

        if let Ok(files_to_delete) = expired_files {
            for file in files_to_delete {
                // Decrypt file path for deletion
                let decrypted_path = match decrypt_string(&cipher, &file.file_path) {
                    Ok(path) => path,
                    Err(e) => {
                        log::error!("Failed to decrypt file path for cleanup: {}", e);
                        continue;
                    }
                };

                log::info!("Deleting expired file: file_id={}", file.file_id);

                // Delete the encrypted file from disk
                if let Err(e) = std::fs::remove_file(&decrypted_path) {
                    log::error!("Failed to delete file {}: {}", decrypted_path, e);
                }

                // Delete from database (this will cascade to access logs)
                if let Err(e) = files::Entity::delete_by_id(file.file_id).exec(&db).await {
                    log::error!("Failed to delete file record {}: {}", file.file_id, e);
                }
            }
        }

        // Clean up old access logs
        let log_retention_cutoff = now - Duration::days(access_log_retention_days);
        let old_logs_result = file_access_logs::Entity::delete_many()
            .filter(file_access_logs::Column::AccessTime.lt(log_retention_cutoff))
            .exec(&db)
            .await;

        match old_logs_result {
            Ok(result) => {
                if result.rows_affected > 0 {
                    log::info!(
                        "Deleted {} old access log entries (older than {} days)",
                        result.rows_affected,
                        access_log_retention_days
                    );
                }
            }
            Err(e) => log::error!("Failed to clean up old access logs: {}", e),
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    if *USING_DEFAULT_KEY {
        log::warn!(
            "ENCRYPTION_KEY is not set, so the built-in default key is in use. That \
             key is published in this repository, so encryption at rest currently \
             protects nothing: anyone who obtains the database or the upload \
             directory can decrypt every file, and can confirm a guessed IP address \
             against the stored digests. Generate a real key with \
             `openssl rand -base64 24` and set ENCRYPTION_KEY before storing \
             anything you care about. Changing it later makes existing uploads \
             permanently undecryptable."
        );
    }

    // A short key was previously accepted and NUL-padded to 32 bytes, silently
    // reducing AES-256 to whatever entropy was supplied. Refuse to start instead:
    // this is a misconfiguration, and running on it produces data that looks
    // encrypted but is not.
    if !*USING_DEFAULT_KEY && ENCRYPTION_KEY.len() < ENCRYPTION_KEY_LEN {
        log::error!(
            "ENCRYPTION_KEY is {} bytes, but {} are required. Shorter values used to \
             be zero-padded, which silently weakened encryption to the entropy you \
             supplied. Generate a key with `openssl rand -base64 24` — 24 random \
             bytes encode to exactly {} characters. Refusing to start.",
            ENCRYPTION_KEY.len(),
            ENCRYPTION_KEY_LEN,
            ENCRYPTION_KEY_LEN
        );
        std::process::exit(1);
    }

    // Longer is not stronger: everything past 32 bytes is discarded below.
    if ENCRYPTION_KEY.len() > ENCRYPTION_KEY_LEN {
        log::warn!(
            "ENCRYPTION_KEY is {} bytes; only the first {} are used and the rest are \
             discarded. `openssl rand -base64 32` produces 44 characters and hits \
             this — use `openssl rand -base64 24` for exactly {}.",
            ENCRYPTION_KEY.len(),
            ENCRYPTION_KEY_LEN,
            ENCRYPTION_KEY_LEN
        );
    }

    // Create encryption key (must be 32 bytes for AES-256)
    let mut encryption_key = [0u8; 32];
    let key_bytes = ENCRYPTION_KEY.as_bytes();
    let copy_len = key_bytes.len().min(32);
    encryption_key[..copy_len].copy_from_slice(&key_bytes[..copy_len]);

    let cipher = Aes256Gcm::new(&encryption_key.into());

    // Setup database
    log::info!("Connecting to database: {}", *DATABASE_URL);

    // Extract database file path and create parent directory if needed
    if DATABASE_URL.starts_with("sqlite:") {
        let db_path = DATABASE_URL
            .strip_prefix("sqlite:")
            .unwrap_or(&DATABASE_URL);
        let db_file = std::path::Path::new(db_path);

        // Create parent directory
        if let Some(parent) = db_file.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create database directory");
        }

        // Create empty database file if it doesn't exist
        if !db_file.exists() {
            std::fs::File::create(db_file).expect("Failed to create database file");
        }
    }

    // Connect to database with connection pooling
    let mut opt = ConnectOptions::new(DATABASE_URL.as_str());
    opt.max_connections(*DB_MAX_CONNECTIONS)
        .min_connections(1)
        .connect_timeout(std::time::Duration::from_secs(8))
        .acquire_timeout(std::time::Duration::from_secs(8))
        // Lifetimes, not timeouts. These previously both read 8 seconds, copied
        // from the two above, which retired every connection almost as fast as
        // it was opened and made the pool reconnect continuously under load.
        .idle_timeout(std::time::Duration::from_secs(300))
        .max_lifetime(std::time::Duration::from_secs(1800));

    let db = sea_orm::Database::connect(opt)
        .await
        .expect("Failed to connect to database");

    init_db(&db).await.expect("Failed to initialize database");
    log::info!("Database initialized successfully");

    // Create upload directory
    std::fs::create_dir_all(UPLOAD_DIR.as_str()).expect("Failed to create upload directory");

    // Spawn background task to clean up expired files
    let cleanup_db = db.clone();
    let cleanup_cipher = cipher.clone();
    tokio::spawn(async move {
        cleanup_expired_files(
            cleanup_db,
            cleanup_cipher,
            *CLEANUP_INTERVAL_SECS,
            *ACCESS_LOG_RETENTION_DAYS,
        )
        .await;
    });

    log::info!(
        "Cleanup task configured to run every {} seconds",
        *CLEANUP_INTERVAL_SECS
    );
    log::info!("Access log retention: {} days", *ACCESS_LOG_RETENTION_DAYS);
    log::info!(
        "Database connection pool: max {} connections",
        *DB_MAX_CONNECTIONS
    );
    log::info!("Server workers: {}", *ACTIX_WORKERS);
    log_inline_policy();

    let app_state = web::Data::new(AppState {
        db,
        upload_dir: PathBuf::from(UPLOAD_DIR.as_str()),
        cipher,
        max_file_size: *MAX_FILE_SIZE,
        file_expiry_minutes: *FILE_EXPIRY_MINUTES,
    });

    let bind_address = format!("{}:{}", *HOST, *PORT);
    log::info!("Starting server at http://{}", bind_address);

    HttpServer::new(move || {
        // Configure CORS with strict policy
        let cors = if let Some(url) = HAKO_BASE_URL.as_ref() {
            // Use custom base URL if provided
            Cors::default()
                .allowed_origin(url)
                .allowed_methods(vec!["GET", "POST"])
                .allowed_headers(vec![
                    actix_web::http::header::CONTENT_TYPE,
                    actix_web::http::header::ACCEPT,
                ])
                .max_age(3600)
        } else {
            // Use default origins for development
            Cors::default()
                .allowed_origin("http://localhost:5173") // Svelte dev server
                .allowed_origin(&format!("http://{}:{}", *HOST, *PORT)) // Production server
                .allowed_methods(vec!["GET", "POST"])
                .allowed_headers(vec![
                    actix_web::http::header::CONTENT_TYPE,
                    actix_web::http::header::ACCEPT,
                ])
                .max_age(3600)
        };

        // Baseline headers on every response. DefaultHeaders only fills in what a
        // handler did not already set, so the download route's own nosniff and
        // sandbox CSP still win on /file/{token}.
        //
        // These four directives are deliberately narrow: they cannot break the
        // SPA. A real script-src is not here because SvelteKit emits inline
        // bootstrap scripts, so it needs hashes generated at build time
        // (`kit.csp = { mode: 'hash' }`) rather than a header we can bolt on.
        let security_headers = middleware::DefaultHeaders::new()
            .add(("X-Content-Type-Options", "nosniff"))
            .add(("X-Frame-Options", "DENY"))
            .add(("Referrer-Policy", "no-referrer"))
            .add((
                "Content-Security-Policy",
                "base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'self'",
            ));

        App::new()
            .wrap(Compress::default()) // Enable gzip compression
            .wrap(security_headers)
            .wrap(cors)
            .app_data(app_state.clone())
            .app_data(web::PayloadConfig::new(100 * 1024 * 1024)) // 100MB max
            .route("/config", web::get().to(get_config))
            .route("/upload", web::post().to(upload_file))
            .route("/file/{token}", web::get().to(get_file))
            .route("/img/{filename}", web::get().to(serve_image))
            .service(
                fs::Files::new("/", "./static")
                    .index_file("index.html")
                    .use_last_modified(true)
                    .default_handler(web::to(|| async {
                        actix_files::NamedFile::open("./static/index.html")
                    })),
            )
    })
    .workers(*ACTIX_WORKERS)
    .bind(&bind_address)?
    .run()
    .await
}

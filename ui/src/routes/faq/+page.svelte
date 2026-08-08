<script lang="ts">
	import { getContext } from 'svelte';

	const maxFileSizeMb = getContext<() => number>('maxFileSizeMb');
	const expiryText = getContext<() => string>('expiryText');
</script>

<div class="card accent-lavender rounded-lg border-2 p-8 text-left">
	<h2 class="mb-6 text-2xl font-semibold">Frequently Asked Questions</h2>

	<div class="space-y-6">
		<!-- Q&A Template -->
		<div class="border-b pb-4" style="border-color: var(--card-divider)">
			<h3 class="mb-2 text-lg font-medium">Why Hako?</h3>
			<p class="text-sm">
				Hako (箱) means "box" in Japanese. This service provides a simple, secure way to share files
				temporarily without long-term storage or complex account setups.
			</p>
		</div>

		<div class="border-b pb-4" style="border-color: var(--card-divider)">
			<h3 class="mb-2 text-lg font-medium">How long are files stored?</h3>
			<p class="text-sm">
				Files are automatically deleted after {expiryText()}. This helps ensure privacy and prevents
				long-term data retention.
			</p>
		</div>

		<div class="border-b pb-4" style="border-color: var(--card-divider)">
			<h3 class="mb-2 text-lg font-medium">What is the maximum file size?</h3>
			<p class="text-sm">
				The current maximum file size is {maxFileSizeMb()} MiB. Files larger than this will be rejected
				during upload.
			</p>
		</div>

		<div class="border-b pb-4" style="border-color: var(--card-divider)">
			<h3 class="mb-2 text-lg font-medium">Are my files encrypted?</h3>
			<p class="text-sm">
				Yes! All files are encrypted at rest using AES-256-GCM encryption. Your files are encrypted
				before being stored on disk and decrypted only when downloaded.
			</p>
		</div>

		<div class="border-b pb-4" style="border-color: var(--card-divider)">
			<h3 class="mb-2 text-lg font-medium">Is my IP address logged?</h3>
			<p class="text-sm">
				IP addresses are one-way hashed using SHA-256. We never store your actual IP address - only
				a cryptographic hash for audit logging purposes.
			</p>
		</div>

		<div class="border-b pb-4" style="border-color: var(--card-divider)">
			<h3 class="mb-2 text-lg font-medium">Can I upload multiple files at once?</h3>
			<p class="text-sm">
				Currently, you can upload one file at a time. Each upload generates a unique hash that can
				be used to download the file later.
			</p>
		</div>

		<div class="pb-4">
			<h3 class="mb-2 text-lg font-medium">What happens to my file after it expires?</h3>
			<p class="text-sm">
				Expired files are permanently deleted from our servers during the next cleanup cycle. Once
				deleted, files cannot be recovered.
			</p>
		</div>
	</div>
</div>

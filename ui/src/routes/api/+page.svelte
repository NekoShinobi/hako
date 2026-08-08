<script lang="ts">
	import { getContext } from 'svelte';

	// Get config from layout context
	const expiryText = getContext<() => string>('expiryText');
</script>

<div class="card accent-teal rounded-lg border-2 p-8 text-left">
	<h2 class="mb-6 text-2xl font-semibold">File Upload</h2>

	<div class="mb-6">
		<h3 class="mb-2 text-lg font-medium">POST /upload</h3>
		<p class="mb-4">
			Upload a file to the service. Files are automatically encrypted and set to expire after {expiryText()}.
		</p>

		<div class="mb-4">
			<h4 class="mb-2 font-medium">Request</h4>
			<p class="mb-2 text-sm">Content-Type: multipart/form-data</p>
			<div class="code-block overflow-x-auto rounded p-4">
				<code class="text-sm">
					curl -X POST https://your-hako-instance.com/upload \<br />
					&nbsp;&nbsp;-F "file=@/path/to/your/file.txt"
				</code>
			</div>
		</div>

		<div class="mb-4">
			<h4 class="mb-2 font-medium">Example Response</h4>
			<div class="code-block overflow-x-auto rounded p-4">
				<pre class="text-sm">{JSON.stringify(
						{
							success: true,
							file_id: 1,
							token: '9f2c41ba7de85310c6a4f0e2b9d7135a',
							hash: 'a40ef1b68f35da42c4c99ad7d2d534715fa81bf46d2dc2a7b0ae19a7cfd8ca91',
							url: '/file/9f2c41ba7de85310c6a4f0e2b9d7135a',
							file_name: 'file.txt',
							size: 114219,
							mime_type: 'text/plain'
						},
						null,
						2
					)}</pre>
			</div>
		</div>
	</div>

	<div class="mb-6">
		<h3 class="mb-2 text-lg font-medium">GET /file/&#123;token&#125;</h3>
		<p class="mb-4">
			Download a file using its token. The file will be decrypted and served with the original
			filename. The token is the only thing protecting the file, so treat it as a secret —
			<code class="font-mono">hash</code> is returned for verification only and is not part of the
			URL.
		</p>

		<div class="mb-4">
			<h4 class="mb-2 font-medium">Request</h4>
			<div class="code-block overflow-x-auto rounded p-4">
				<code class="text-sm">
					curl https://your-hako-instance.com/file/&#123;token&#125; \<br />
					&nbsp;&nbsp;-o downloaded-file.txt
				</code>
			</div>
		</div>

		<div class="mb-4">
			<h4 class="mb-2 font-medium">Error Responses</h4>
			<p class="mb-2 text-sm">404 Not Found - File does not exist</p>
			<p class="text-sm">410 Gone - File has expired</p>
		</div>
	</div>

	<div class="border-t pt-4" style="border-color: var(--card-divider)">
		<p class="text-sm">
			<strong>Note:</strong> All files are encrypted at rest using AES-256-GCM. IP addresses are
			one-way hashed for audit logging. Files are automatically deleted after {expiryText()}.
		</p>
	</div>
</div>

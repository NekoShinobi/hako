<script lang="ts">
	import { onMount } from 'svelte';
	import UploadModal from '$lib/components/UploadModal.svelte';

	type Status = 'idle' | 'uploading' | 'success' | 'error';

	let fileInput: HTMLInputElement | undefined = $state();
	let dragOver = $state(false);
	let modalOpen = $state(false);
	let status = $state<Status>('idle');
	let progress = $state(0);
	let fileName = $state('');
	let fileSize = $state('');
	let uploadedUrl = $state('');
	let errorMessage = $state('');
	let copied = $state(false);

	// Not $state: only ever used to abort, never rendered.
	let request: XMLHttpRequest | null = null;

	function formatFileSize(bytes: number): string {
		if (bytes === 0) return '0 Bytes';
		const k = 1024;
		const sizes = ['Bytes', 'KB', 'MB', 'GB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return Math.round((bytes / k ** i) * 100) / 100 + ' ' + sizes[i];
	}

	function fail(message: string) {
		status = 'error';
		errorMessage = message;
	}

	// XMLHttpRequest rather than fetch: it is the only way to observe upload progress.
	function uploadFile(file: File) {
		fileName = file.name;
		fileSize = formatFileSize(file.size);
		progress = 0;
		uploadedUrl = '';
		errorMessage = '';
		status = 'uploading';
		modalOpen = true;

		const formData = new FormData();
		formData.append('file', file);

		const xhr = new XMLHttpRequest();
		request = xhr;

		xhr.upload.addEventListener('progress', (event) => {
			if (event.lengthComputable) {
				progress = Math.round((event.loaded / event.total) * 100);
			}
		});

		xhr.addEventListener('load', () => {
			if (xhr.status < 200 || xhr.status >= 300) {
				fail(`Upload failed (HTTP ${xhr.status})`);
				return;
			}
			try {
				const data = JSON.parse(xhr.responseText);
				if (data.success) {
					progress = 100;
					uploadedUrl = window.location.origin + data.url;
					status = 'success';
				} else {
					fail(data.error || 'Upload failed');
				}
			} catch {
				fail('Unexpected response from the server');
			}
		});

		xhr.addEventListener('error', () => fail('Network error during upload'));
		xhr.addEventListener('timeout', () => fail('The upload timed out'));
		xhr.addEventListener('abort', () => {
			status = 'idle';
			modalOpen = false;
		});

		xhr.open('POST', '/upload');
		xhr.send(formData);
	}

	function cancelUpload() {
		request?.abort();
		request = null;
	}

	function closeModal() {
		modalOpen = false;
	}

	function handleFileSelect(event: Event) {
		const target = event.target as HTMLInputElement;
		if (target.files && target.files.length > 0) {
			uploadFile(target.files[0]);
			// Let the same file be picked again right afterwards.
			target.value = '';
		}
	}

	function handleDrop(event: DragEvent) {
		event.preventDefault();
		dragOver = false;
		if (event.dataTransfer?.files && event.dataTransfer.files.length > 0) {
			uploadFile(event.dataTransfer.files[0]);
		}
	}

	function handleDragOver(event: DragEvent) {
		event.preventDefault();
		dragOver = true;
	}

	function handleDragLeave() {
		dragOver = false;
	}

	// The drop zone is a div with role="button", so it has to answer the keys a
	// real button would.
	function handleDropZoneKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			fileInput?.click();
		}
	}

	function handlePaste(event: ClipboardEvent) {
		if (status === 'uploading') return;
		const items = event.clipboardData?.items;
		if (items) {
			for (let i = 0; i < items.length; i++) {
				if (items[i].kind === 'file') {
					const file = items[i].getAsFile();
					if (file) {
						uploadFile(file);
						break;
					}
				}
			}
		}
	}

	async function copyToClipboard() {
		if (!uploadedUrl) return;
		try {
			await navigator.clipboard.writeText(uploadedUrl);
			copied = true;
			setTimeout(() => {
				copied = false;
			}, 2000);
		} catch (error) {
			console.error('Failed to copy:', error);
		}
	}

	onMount(() => {
		document.addEventListener('paste', handlePaste);
		return () => {
			document.removeEventListener('paste', handlePaste);
		};
	});
</script>

<div
	class="card accent-green cursor-pointer rounded-lg border-2 p-16 transition-colors"
	style={dragOver ? 'background-color: var(--card-bg-drag);' : ''}
	onclick={() => fileInput?.click()}
	onkeydown={handleDropZoneKeydown}
	ondrop={handleDrop}
	ondragover={handleDragOver}
	ondragleave={handleDragLeave}
	role="button"
	tabindex="0"
>
	<p class="text-xl">Drop or paste file(s)</p>
</div>

<input type="file" bind:this={fileInput} onchange={handleFileSelect} class="hidden" />

<!-- Stays behind after the modal is dismissed, so the link is never lost. -->
{#if status === 'success' && !modalOpen}
	<div class="mt-6 rounded-lg p-4" style="background-color: var(--color-success-bg)">
		<div class="flex items-center justify-center gap-2">
			<!-- Points at the backend's file-download endpoint, not a SvelteKit route, so resolve() does not apply. -->
			<!-- eslint-disable svelte/no-navigation-without-resolve -->
			<a
				href={uploadedUrl}
				target="_blank"
				rel="noopener noreferrer"
				style="color: var(--color-primary-blue)"
				class="break-all hover:underline"
			>
				{uploadedUrl}
			</a>
			<!-- eslint-enable svelte/no-navigation-without-resolve -->
			<button
				onclick={copyToClipboard}
				style="background-color: var(--color-primary-blue); color: var(--color-on-primary)"
				class="rounded px-3 py-1 text-sm transition-all hover:opacity-90"
				class:opacity-90={copied}
			>
				{copied ? 'Copied!' : 'Copy'}
			</button>
		</div>
	</div>
{/if}

{#if modalOpen}
	<UploadModal
		status={status === 'idle' ? 'uploading' : status}
		{progress}
		{fileName}
		{fileSize}
		url={uploadedUrl}
		error={errorMessage}
		onclose={closeModal}
		oncancel={cancelUpload}
	/>
{/if}

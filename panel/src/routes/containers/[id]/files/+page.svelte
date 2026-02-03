<script lang="ts">
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { getContext, onMount } from 'svelte';
    import type { Writable } from 'svelte/store';
    import { api } from '$lib/api';
    import toast from 'svelte-french-toast';
    import { _ } from '$lib/i18n';
    import type { Container } from '$lib/types';

    interface FileEntry {
        name: string;
        isDir: boolean;
        size: number;
        modified?: string;
    }

    const containerStore = getContext<Writable<Container | null>>('container');

    let files: FileEntry[] = [];
    let currentPath = '/';
    let loadingFiles = false;
    let selectedFiles: Set<string> = new Set();
    let showNewFolderModal = false;
    let newFolderName = '';
    let creatingFolder = false;
    let showDeleteModal = false;
    let deleting = false;

    let isDragging = false;
    let dragCounter = 0;
    let uploading = false;
    let uploadProgress = 0;

    let selectMode = false;

    let showNavSidebar = true;

    $: containerId = $page.params.id as string;
    $: container = $containerStore;

    $: if (selectedFiles.size === 0 && selectMode) {
    }

    onMount(() => {
        loadFiles('/');
    });

    async function loadFiles(path: string) {
        loadingFiles = true;
        selectedFiles = new Set();
        selectMode = false;
        try {
            files = await api.listContainerFiles(containerId, path);
            currentPath = path;
        } catch (e: any) {
            toast.error(e.message || 'Failed to load files');
        } finally {
            loadingFiles = false;
        }
    }

    function navigateTo(file: FileEntry) {
        if (file.isDir) {
            const newPath = currentPath === '/' ? `/${file.name}` : `${currentPath}/${file.name}`;
            loadFiles(newPath);
        } else {
            const encodedPath = encodeURIComponent(currentPath === '/' ? file.name : `${currentPath.slice(1)}/${file.name}`);
            goto(`/containers/${containerId}/files/${encodedPath}`);
        }
    }

    function handleFileClick(file: FileEntry, event: MouseEvent) {
        if (selectMode && !file.isDir) {
            toggleFileSelection(file.name, event);
        } else {
            navigateTo(file);
        }
    }

    function handleCheckboxClick(file: FileEntry, event: MouseEvent) {
        event.stopPropagation();
        toggleFileSelection(file.name, event);
    }

    function goUp() {
        const parts = currentPath.split('/').filter(Boolean);
        parts.pop();
        loadFiles(parts.length ? '/' + parts.join('/') : '/');
    }

    function goToPath(path: string) {
        loadFiles(path);
    }

    function formatSize(bytes: number): string {
        if (bytes === 0) return '-';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
    }

    function formatDate(dateStr?: string): string {
        if (!dateStr) return '-';
        return new Date(dateStr).toLocaleString();
    }

    function toggleFileSelection(fileName: string, event: MouseEvent) {
        if (event.shiftKey && selectedFiles.size > 0) {
            const fileNames = files.map(f => f.name);
            const lastSelected = Array.from(selectedFiles).pop()!;
            const lastIndex = fileNames.indexOf(lastSelected);
            const currentIndex = fileNames.indexOf(fileName);
            const start = Math.min(lastIndex, currentIndex);
            const end = Math.max(lastIndex, currentIndex);
            for (let i = start; i <= end; i++) {
                selectedFiles.add(fileNames[i]);
            }
            selectedFiles = selectedFiles;
        } else if (event.ctrlKey || event.metaKey) {
            if (selectedFiles.has(fileName)) {
                selectedFiles.delete(fileName);
            } else {
                selectedFiles.add(fileName);
            }
            selectedFiles = selectedFiles;
        } else {
            if (selectedFiles.has(fileName)) {
                selectedFiles.delete(fileName);
            } else {
                selectedFiles.add(fileName);
            }
            selectedFiles = selectedFiles;
        }
    }

    function selectAll() {
        selectedFiles = new Set(files.map(f => f.name));
    }

    function clearSelection() {
        selectedFiles = new Set();
    }

    function toggleSelectMode() {
        selectMode = !selectMode;
        if (!selectMode) {
            selectedFiles = new Set();
        }
    }

    function exitSelectMode() {
        selectMode = false;
        selectedFiles = new Set();
    }

    function handleDragEnter(event: DragEvent) {
        event.preventDefault();
        dragCounter++;
        isDragging = true;
    }

    function handleDragOver(event: DragEvent) {
        event.preventDefault();
    }

    function handleDragLeave(event: DragEvent) {
        event.preventDefault();
        dragCounter--;
        if (dragCounter === 0) {
            isDragging = false;
        }
    }

    let currentUploadFile = '';
    let currentFileProgress = 0;

    async function handleDrop(event: DragEvent) {
        event.preventDefault();
        dragCounter = 0;
        isDragging = false;

        const items = event.dataTransfer?.files;
        if (!items || items.length === 0) return;

        uploading = true;
        uploadProgress = 0;
        currentFileProgress = 0;

        try {
            const total = items.length;
            let completed = 0;

            for (const file of Array.from(items)) {
                currentUploadFile = file.name;
                const path = currentPath === '/' ? file.name : `${currentPath}/${file.name}`;
                await api.uploadContainerFile(containerId, path.startsWith('/') ? path.slice(1) : path, file, (progress) => {
                    currentFileProgress = progress;
                    uploadProgress = Math.round(((completed + (progress / 100)) / total) * 100);
                });
                completed++;
                uploadProgress = Math.round((completed / total) * 100);
            }

            toast.success(`Uploaded ${total} file(s)`);
            await loadFiles(currentPath);
        } catch (e: any) {
            toast.error(e.message || 'Failed to upload files');
        } finally {
            uploading = false;
            uploadProgress = 0;
            currentUploadFile = '';
            currentFileProgress = 0;
        }
    }

    async function createFolder() {
        if (!newFolderName.trim()) return;
        creatingFolder = true;
        try {
            const path = currentPath === '/' ? newFolderName : `${currentPath}/${newFolderName}`;
            await api.createContainerFolder(containerId, path.startsWith('/') ? path.slice(1) : path);
            toast.success('Folder created');
            showNewFolderModal = false;
            newFolderName = '';
            await loadFiles(currentPath);
        } catch (e: any) {
            toast.error(e.message || 'Failed to create folder');
        } finally {
            creatingFolder = false;
        }
    }

    async function deleteSelected() {
        if (selectedFiles.size === 0) return;
        deleting = true;
        try {
            for (const fileName of selectedFiles) {
                const path = currentPath === '/' ? fileName : `${currentPath}/${fileName}`;
                await api.deleteContainerFile(containerId, path.startsWith('/') ? path.slice(1) : path);
            }
            toast.success(`Deleted ${selectedFiles.size} item(s)`);
            showDeleteModal = false;
            selectMode = false;
            await loadFiles(currentPath);
        } catch (e: any) {
            toast.error(e.message || 'Failed to delete files');
        } finally {
            deleting = false;
        }
    }

    let fileInput: HTMLInputElement;

    function triggerUpload() {
        fileInput?.click();
    }

    async function handleFileInput(event: Event) {
        const input = event.target as HTMLInputElement;
        const items = input.files;
        if (!items || items.length === 0) return;

        uploading = true;
        uploadProgress = 0;
        currentFileProgress = 0;

        try {
            const total = items.length;
            let completed = 0;

            for (const file of Array.from(items)) {
                currentUploadFile = file.name;
                const path = currentPath === '/' ? file.name : `${currentPath}/${file.name}`;
                await api.uploadContainerFile(containerId, path.startsWith('/') ? path.slice(1) : path, file, (progress) => {
                    currentFileProgress = progress;
                    uploadProgress = Math.round(((completed + (progress / 100)) / total) * 100);
                });
                completed++;
                uploadProgress = Math.round((completed / total) * 100);
            }

            toast.success(`Uploaded ${total} file(s)`);
            await loadFiles(currentPath);
        } catch (e: any) {
            toast.error(e.message || 'Failed to upload files');
        } finally {
            uploading = false;
            uploadProgress = 0;
            currentUploadFile = '';
            currentFileProgress = 0;
            input.value = '';
        }
    }

    function getFileType(fileName: string): string {
        const ext = fileName.split('.').pop()?.toLowerCase() || '';
        const types: Record<string, string> = {
            'jar': 'java', 'java': 'java', 'class': 'java',
            'js': 'code', 'ts': 'code', 'jsx': 'code', 'tsx': 'code', 'vue': 'code', 'svelte': 'code',
            'json': 'data', 'xml': 'data', 'csv': 'data',
            'yml': 'config', 'yaml': 'config', 'properties': 'config', 'conf': 'config', 'toml': 'config', 'ini': 'config', 'env': 'config',
            'txt': 'text', 'md': 'text', 'log': 'text', 'readme': 'text',
            'png': 'image', 'jpg': 'image', 'jpeg': 'image', 'gif': 'image', 'ico': 'image', 'svg': 'image', 'webp': 'image',
            'zip': 'archive', 'tar': 'archive', 'gz': 'archive', 'rar': 'archive', '7z': 'archive',
            'sh': 'script', 'bat': 'script', 'cmd': 'script', 'ps1': 'script',
            'html': 'web', 'css': 'web', 'scss': 'web', 'less': 'web',
            'sql': 'database', 'db': 'database', 'sqlite': 'database',
        };
        return types[ext] || 'default';
    }

    $: breadcrumbs = currentPath === '/' ? [] : currentPath.split('/').filter(Boolean).map((part, i, arr) => ({
        name: part,
        path: '/' + arr.slice(0, i + 1).join('/')
    }));
</script>

<div class="h-full flex flex-col"
     on:dragenter={handleDragEnter}
     on:dragover={handleDragOver}
     on:dragleave={handleDragLeave}
     on:drop={handleDrop}
     role="region"
     aria-label="File manager">

    <!-- Toolbar -->
    <div class="flex-shrink-0 border-b border-dark-700 bg-dark-900 p-2 md:p-3">
        <div class="flex flex-col md:flex-row md:items-center justify-between gap-2 md:gap-4">
            <!-- Breadcrumb Navigation -->
            <div class="flex items-center gap-1 md:gap-2 min-w-0 flex-1 overflow-x-auto">
                <button on:click={() => showNavSidebar = !showNavSidebar} class="p-1.5 rounded hover:bg-dark-700 text-dark-400 hover:text-white flex-shrink-0" title="Toggle sidebar">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h16" />
                    </svg>
                </button>
                {#if currentPath !== '/'}
                    <button on:click={goUp} class="p-1.5 rounded hover:bg-dark-700 text-dark-400 hover:text-white flex-shrink-0" title="Go up">
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" />
                        </svg>
                    </button>
                {/if}
                <div class="flex items-center gap-1 text-sm overflow-x-auto flex-1 min-w-0 scrollbar-hide">
                    <button on:click={() => goToPath('/')} class="text-dark-400 hover:text-white px-1 flex-shrink-0">
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12l8.954-8.955c.44-.439 1.152-.439 1.591 0L21.75 12M4.5 9.75v10.125c0 .621.504 1.125 1.125 1.125H9.75v-4.875c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125V21h4.125c.621 0 1.125-.504 1.125-1.125V9.75M8.25 21h8.25" />
                        </svg>
                    </button>
                    {#each breadcrumbs as crumb, i}
                        <span class="text-dark-600">/</span>
                        <button on:click={() => goToPath(crumb.path)} class="text-dark-400 hover:text-white truncate max-w-[100px]">{crumb.name}</button>
                    {/each}
                </div>
            </div>

            <!-- Actions -->
            <div class="flex items-center gap-1 md:gap-2 flex-shrink-0 flex-wrap justify-end">
                {#if selectMode}
                    <span class="text-xs md:text-sm text-dark-400">{selectedFiles.size}</span>
                    {#if selectedFiles.size > 0}
                        <button on:click={() => showDeleteModal = true} class="btn-danger text-xs md:text-sm px-2 py-1 md:px-3 md:py-1.5">
                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
                            </svg>
                            <span class="hidden sm:inline ml-1">Delete</span>
                        </button>
                    {/if}
                    <button on:click={selectAll} class="btn-ghost text-xs md:text-sm px-2 py-1 md:px-3 md:py-1.5">All</button>
                    <button on:click={exitSelectMode} class="btn-secondary text-xs md:text-sm px-2 py-1 md:px-3 md:py-1.5">
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                {:else}
                    <button on:click={toggleSelectMode} class="btn-ghost text-xs md:text-sm px-2 py-1 md:px-3 md:py-1.5" title="Select files">
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        <span class="hidden sm:inline ml-1">Select</span>
                    </button>
                    <button on:click={() => showNewFolderModal = true} class="btn-secondary text-xs md:text-sm px-2 py-1 md:px-3 md:py-1.5">
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                        </svg>
                        <span class="hidden sm:inline ml-1">Folder</span>
                    </button>
                    <button on:click={triggerUpload} disabled={uploading} class="btn-primary text-xs md:text-sm px-2 py-1 md:px-3 md:py-1.5 relative">
                        {#if uploading}
                            <span class="spinner w-4 h-4"></span>
                            <span class="ml-1">{uploadProgress}%</span>
                        {:else}
                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5" />
                            </svg>
                            <span class="hidden sm:inline ml-1">Upload</span>
                        {/if}
                    </button>
                {/if}
                <input bind:this={fileInput} type="file" multiple accept="*/*" class="hidden" on:change={handleFileInput} />
            </div>
        </div>
    </div>

    <!-- File List -->
    <div class="flex-1 overflow-y-auto relative {isDragging ? 'bg-primary-900/10' : ''}">
        {#if isDragging}
            <div class="absolute inset-0 flex items-center justify-center bg-dark-900/80 z-10 border-2 border-dashed border-primary-500 rounded-lg m-2">
                <div class="text-center">
                    <svg class="w-12 h-12 mx-auto text-primary-400 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5" />
                    </svg>
                    <p class="text-primary-400 font-medium">Drop files to upload</p>
                </div>
            </div>
        {/if}

        {#if loadingFiles}
            <div class="p-8 text-center">
                <div class="spinner w-6 h-6 mx-auto mb-2"></div>
                <p class="text-dark-400 text-sm">Loading files...</p>
            </div>
        {:else if files.length === 0}
            <div class="p-8 text-center">
                <svg class="w-12 h-12 mx-auto text-dark-600 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z" />
                </svg>
                <p class="text-dark-400">This folder is empty</p>
                <p class="text-dark-500 text-sm mt-1">Drag and drop files here to upload</p>
            </div>
        {:else}
            <div class="divide-y divide-dark-700/50">
                {#each files as file}
                    <div
                        class="w-full flex items-center gap-2 md:gap-3 px-2 md:px-4 py-2 md:py-2.5 hover:bg-dark-700/30 transition-colors text-left group {selectedFiles.has(file.name) ? 'bg-primary-900/20' : ''}"
                        role="button"
                        tabindex="0"
                        on:click={(e) => handleFileClick(file, e)}
                        on:keydown={(e) => e.key === 'Enter' && navigateTo(file)}
                    >
                        {#if selectMode}
                            <button
                                class="flex-shrink-0"
                                on:click|stopPropagation={(e) => handleCheckboxClick(file, e)}
                            >
                                <div class="w-5 h-5 rounded border-2 flex items-center justify-center transition-colors {selectedFiles.has(file.name) ? 'bg-primary-500 border-primary-500' : 'border-dark-500 group-hover:border-dark-400'}">
                                    {#if selectedFiles.has(file.name)}
                                        <svg class="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                                        </svg>
                                    {/if}
                                </div>
                            </button>
                        {/if}

                        <!-- File Icon -->
                        <div class="flex-shrink-0 w-6 h-6 md:w-8 md:h-8 flex items-center justify-center cursor-pointer">
                            {#if file.isDir}
                                <svg class="w-5 h-5 md:w-7 md:h-7 text-amber-400" fill="currentColor" viewBox="0 0 24 24">
                                    <path d="M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
                                </svg>
                            {:else}
                                {@const fileType = getFileType(file.name)}
                                {#if fileType === 'java'}
                                    <svg class="w-6 h-6 text-orange-400" fill="currentColor" viewBox="0 0 24 24">
                                        <path d="M8.851 18.56s-.917.534.653.714c1.902.218 2.874.187 4.969-.211 0 0 .552.346 1.321.646-4.699 2.013-10.633-.118-6.943-1.149M8.276 15.933s-1.028.761.542.924c2.032.209 3.636.227 6.413-.308 0 0 .384.389.987.602-5.679 1.661-12.007.13-7.942-1.218M13.116 11.475c1.158 1.333-.304 2.533-.304 2.533s2.939-1.518 1.589-3.418c-1.261-1.772-2.228-2.652 3.007-5.688 0-.001-8.216 2.051-4.292 6.573M19.33 20.504s.679.559-.747.991c-2.712.822-11.288 1.069-13.669.033-.856-.373.75-.89 1.254-.998.527-.114.828-.093.828-.093-.953-.671-6.156 1.317-2.643 1.887 9.58 1.553 17.462-.7 14.977-1.82M9.292 13.21s-4.362 1.036-1.544 1.412c1.189.159 3.561.123 5.77-.062 1.806-.152 3.618-.477 3.618-.477s-.637.272-1.098.587c-4.429 1.165-12.986.623-10.522-.568 2.082-1.006 3.776-.892 3.776-.892M17.116 17.584c4.503-2.34 2.421-4.589.968-4.285-.355.074-.515.138-.515.138s.132-.207.385-.297c2.875-1.011 5.086 2.981-.928 4.562 0-.001.07-.062.09-.118M14.401 0s2.494 2.494-2.365 6.33c-3.896 3.077-.888 4.832-.001 6.836-2.274-2.053-3.943-3.858-2.824-5.539 1.644-2.469 6.197-3.665 5.19-7.627"/>
                                    </svg>
                                {:else if fileType === 'code'}
                                    <svg class="w-6 h-6 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M17.25 6.75L22.5 12l-5.25 5.25m-10.5 0L1.5 12l5.25-5.25m7.5-3l-4.5 16.5" />
                                    </svg>
                                {:else if fileType === 'config'}
                                    <svg class="w-6 h-6 text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z" />
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                                    </svg>
                                {:else if fileType === 'data'}
                                    <svg class="w-6 h-6 text-yellow-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M17.593 3.322c1.1.128 1.907 1.077 1.907 2.185V21L12 17.25 4.5 21V5.507c0-1.108.806-2.057 1.907-2.185a48.507 48.507 0 0111.186 0z" />
                                    </svg>
                                {:else if fileType === 'image'}
                                    <svg class="w-6 h-6 text-pink-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 15.75l5.159-5.159a2.25 2.25 0 013.182 0l5.159 5.159m-1.5-1.5l1.409-1.409a2.25 2.25 0 013.182 0l2.909 2.909m-18 3.75h16.5a1.5 1.5 0 001.5-1.5V6a1.5 1.5 0 00-1.5-1.5H3.75A1.5 1.5 0 002.25 6v12a1.5 1.5 0 001.5 1.5zm10.5-11.25h.008v.008h-.008V8.25zm.375 0a.375.375 0 11-.75 0 .375.375 0 01.75 0z" />
                                    </svg>
                                {:else if fileType === 'archive'}
                                    <svg class="w-6 h-6 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M20.25 7.5l-.625 10.632a2.25 2.25 0 01-2.247 2.118H6.622a2.25 2.25 0 01-2.247-2.118L3.75 7.5M10 11.25h4M3.375 7.5h17.25c.621 0 1.125-.504 1.125-1.125v-1.5c0-.621-.504-1.125-1.125-1.125H3.375c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125z" />
                                    </svg>
                                {:else if fileType === 'script'}
                                    <svg class="w-6 h-6 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M6.75 7.5l3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0021 18V6a2.25 2.25 0 00-2.25-2.25H5.25A2.25 2.25 0 003 6v12a2.25 2.25 0 002.25 2.25z" />
                                    </svg>
                                {:else if fileType === 'text'}
                                    <svg class="w-6 h-6 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" />
                                    </svg>
                                {:else if fileType === 'web'}
                                    <svg class="w-6 h-6 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 21a9.004 9.004 0 008.716-6.747M12 21a9.004 9.004 0 01-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 017.843 4.582M12 3a8.997 8.997 0 00-7.843 4.582m15.686 0A11.953 11.953 0 0112 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0121 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0112 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 013 12c0-1.605.42-3.113 1.157-4.418" />
                                    </svg>
                                {:else if fileType === 'database'}
                                    <svg class="w-6 h-6 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375m16.5 0v3.75m-16.5-3.75v3.75m16.5 0v3.75C20.25 16.153 16.556 18 12 18s-8.25-1.847-8.25-4.125v-3.75m16.5 0c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125" />
                                    </svg>
                                {:else}
                                    <svg class="w-6 h-6 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" />
                                    </svg>
                                {/if}
                            {/if}
                        </div>

                        <!-- File Name -->
                        <div class="flex-1 min-w-0">
                            <span class="text-white truncate block text-sm md:text-base">{file.name}</span>
                        </div>

                        <!-- File Size -->
                        <div class="flex-shrink-0 text-xs md:text-sm text-dark-400 font-mono w-14 md:w-20 text-right">
                            {file.isDir ? '-' : formatSize(file.size)}
                        </div>

                        <!-- Modified Date -->
                        <div class="flex-shrink-0 text-xs md:text-sm text-dark-400 w-32 md:w-40 text-right hidden lg:block">
                            {formatDate(file.modified)}
                        </div>

                        <!-- Chevron for folders -->
                        {#if file.isDir}
                            <div class="flex-shrink-0 text-dark-500 group-hover:text-dark-300">
                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" />
                                </svg>
                            </div>
                        {/if}
                    </div>
                {/each}
            </div>
        {/if}
    </div>
</div>

<!-- New Folder Modal -->
{#if showNewFolderModal}
    <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
        <div class="bg-dark-800 rounded-lg w-full max-w-md">
            <div class="p-4 border-b border-dark-700">
                <h2 class="text-lg font-semibold text-white">New Folder</h2>
            </div>
            <form on:submit|preventDefault={createFolder} class="p-4">
                <input type="text" bind:value={newFolderName} placeholder="Folder name" class="input w-full" autofocus />
                <div class="flex justify-end gap-2 mt-4">
                    <button type="button" on:click={() => { showNewFolderModal = false; newFolderName = ''; }} class="btn-secondary">Cancel</button>
                    <button type="submit" disabled={creatingFolder || !newFolderName.trim()} class="btn-primary">
                        {#if creatingFolder}<span class="spinner w-4 h-4"></span>{:else}Create{/if}
                    </button>
                </div>
            </form>
        </div>
    </div>
{/if}

<!-- Delete Confirmation Modal -->
{#if showDeleteModal}
    <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
        <div class="bg-dark-800 rounded-lg w-full max-w-md">
            <div class="p-4 border-b border-dark-700">
                <h2 class="text-lg font-semibold text-white">Delete Files</h2>
            </div>
            <div class="p-4">
                <p class="text-dark-300 mb-4">
                    Are you sure you want to delete {selectedFiles.size} item(s)? This action cannot be undone.
                </p>
                <div class="bg-dark-900 rounded p-3 max-h-32 overflow-y-auto text-sm font-mono text-dark-400">
                    {#each Array.from(selectedFiles) as fileName}
                        <div class="truncate">{fileName}</div>
                    {/each}
                </div>
                <div class="flex justify-end gap-2 mt-4">
                    <button type="button" on:click={() => showDeleteModal = false} class="btn-secondary">Cancel</button>
                    <button on:click={deleteSelected} disabled={deleting} class="btn-danger">
                        {#if deleting}<span class="spinner w-4 h-4"></span>{:else}Delete{/if}
                    </button>
                </div>
            </div>
        </div>
    </div>
{/if}

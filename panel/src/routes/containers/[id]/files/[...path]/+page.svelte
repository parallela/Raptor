<script lang="ts">
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { getContext, onMount, onDestroy, tick } from 'svelte';
    import type { Writable } from 'svelte/store';
    import { api } from '$lib/api';
    import toast from 'svelte-french-toast';
    import type { Container } from '$lib/types';

    const containerStore = getContext<Writable<Container | null>>('container');

    let fileContent = '';
    let originalContent = '';
    let loading = true;
    let saving = false;
    let hasChanges = false;
    let editorReady = false;
    let mounted = false;

    let editorContainer: HTMLDivElement;
    let editor: any;
    let monaco: any;

    $: containerId = $page.params.id as string;
    $: filePath = decodeURIComponent($page.params.path || '');
    $: fileName = filePath.split('/').pop() || '';
    $: container = $containerStore;
    $: hasChanges = fileContent !== originalContent;

    $: language = getLanguage(fileName);

    $: if (editor && monaco && language) {
        const model = editor?.getModel();
        if (model) {
            monaco.editor.setModelLanguage(model, language);
        }
    }

    onMount(async () => {
        mounted = true;
        await loadFile();
        await tick();
        if (editorContainer && !editorReady) {
            await initMonaco();
        }
    });

    onDestroy(() => {
        mounted = false;
        if (editor) {
            editor.dispose();
            editor = null;
        }
        editorReady = false;
    });

    async function initMonaco() {
        if (editorReady || !editorContainer || !mounted) return;

        try {
            await tick();

            monaco = await import('monaco-editor');

            monaco.editor.defineTheme('raptor-dark', {
                base: 'vs-dark',
                inherit: true,
                rules: [],
                colors: {
                    'editor.background': '#0a0a0f',
                    'editor.lineHighlightBackground': '#1a1a2e',
                    'editorLineNumber.foreground': '#4a5568',
                    'editorLineNumber.activeForeground': '#a0aec0',
                }
            });

            editor = monaco.editor.create(editorContainer, {
                value: fileContent,
                language: language,
                theme: 'raptor-dark',
                automaticLayout: true,
                minimap: { enabled: true, scale: 1 },
                fontSize: 14,
                fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', 'Monaco', monospace",
                lineNumbers: 'on',
                wordWrap: 'off',
                scrollBeyondLastLine: false,
                renderLineHighlight: 'line',
                selectOnLineNumbers: true,
                roundedSelection: true,
                readOnly: false,
                cursorStyle: 'line',
                contextmenu: true,
                smoothScrolling: true,
                cursorBlinking: 'smooth',
                scrollbar: {
                    vertical: 'visible',
                    horizontal: 'auto',
                    useShadows: false,
                    verticalScrollbarSize: 10,
                    horizontalScrollbarSize: 10,
                },
                overviewRulerBorder: false,
                padding: { top: 16, bottom: 16 },
                bracketPairColorization: { enabled: true },
                guides: {
                    bracketPairs: true,
                    indentation: true,
                },
            });

            editor.onDidChangeModelContent(() => {
                fileContent = editor.getValue();
            });

            editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
                if (hasChanges) saveFile();
            });

            editorReady = true;
        } catch (e) {
            console.error('Failed to load Monaco editor:', e);
        }
    }

    async function loadFile() {
        loading = true;
        try {
            fileContent = await api.getContainerFile(containerId, filePath);
            originalContent = fileContent;
            if (editor) {
                editor.setValue(fileContent);
            }
        } catch (e: any) {
            toast.error(e.message || 'Failed to load file');
            goBack();
        } finally {
            loading = false;
        }
    }

    async function saveFile() {
        saving = true;
        try {
            await api.saveContainerFile(containerId, filePath, fileContent);
            originalContent = fileContent;
            toast.success('File saved');
        } catch (e: any) {
            toast.error(e.message || 'Failed to save file');
        } finally {
            saving = false;
        }
    }

    function goBack() {
        const parts = filePath.split('/');
        parts.pop();
        const parentPath = parts.join('/');
        goto(`/containers/${containerId}/files${parentPath ? '?path=' + encodeURIComponent('/' + parentPath) : ''}`);
    }

    function handleKeydown(event: KeyboardEvent) {
        if ((event.ctrlKey || event.metaKey) && event.key === 's') {
            event.preventDefault();
            if (hasChanges) saveFile();
        }
    }

    function getLanguage(name: string): string {
        const ext = name.split('.').pop()?.toLowerCase();
        const langMap: Record<string, string> = {
            'js': 'javascript',
            'mjs': 'javascript',
            'cjs': 'javascript',
            'ts': 'typescript',
            'tsx': 'typescript',
            'jsx': 'javascript',
            'json': 'json',
            'yml': 'yaml',
            'yaml': 'yaml',
            'properties': 'ini',
            'conf': 'ini',
            'cfg': 'ini',
            'ini': 'ini',
            'env': 'shell',
            'sh': 'shell',
            'bash': 'shell',
            'zsh': 'shell',
            'bat': 'bat',
            'cmd': 'bat',
            'ps1': 'powershell',
            'xml': 'xml',
            'html': 'html',
            'htm': 'html',
            'css': 'css',
            'scss': 'scss',
            'less': 'less',
            'md': 'markdown',
            'markdown': 'markdown',
            'py': 'python',
            'python': 'python',
            'java': 'java',
            'kt': 'kotlin',
            'kts': 'kotlin',
            'groovy': 'groovy',
            'gradle': 'groovy',
            'toml': 'toml',
            'rs': 'rust',
            'go': 'go',
            'rb': 'ruby',
            'php': 'php',
            'sql': 'sql',
            'c': 'c',
            'cpp': 'cpp',
            'h': 'c',
            'hpp': 'cpp',
            'cs': 'csharp',
            'svelte': 'html',
            'vue': 'html',
            'dockerfile': 'dockerfile',
            'makefile': 'makefile',
            'log': 'plaintext',
            'txt': 'plaintext',
        };
        return langMap[ext || ''] || 'plaintext';
    }

    function getFileIcon(name: string): string {
        const ext = name.split('.').pop()?.toLowerCase();
        if (ext === 'jar' || ext === 'java' || ext === 'class') return 'java';
        if (['js', 'ts', 'jsx', 'tsx', 'mjs', 'cjs'].includes(ext || '')) return 'code';
        if (['yml', 'yaml', 'properties', 'conf', 'toml', 'ini', 'env'].includes(ext || '')) return 'config';
        if (ext === 'json' || ext === 'xml') return 'data';
        if (['md', 'txt', 'log'].includes(ext || '')) return 'text';
        return 'file';
    }

    $: breadcrumbs = filePath.split('/').filter(Boolean).map((part, i, arr) => ({
        name: part,
        path: arr.slice(0, i + 1).join('/'),
        isFile: i === arr.length - 1
    }));
</script>

<svelte:head>
    <title>{fileName || 'File'} - {container?.name || 'Container'} - Raptor</title>
</svelte:head>

<svelte:window on:keydown={handleKeydown} />

<div class="h-full flex flex-col bg-dark-900">
    <!-- Header -->
    <div class="flex-shrink-0 border-b border-dark-700 bg-dark-900 p-3">
        <div class="flex items-center justify-between gap-4">
            <div class="flex items-center gap-3 min-w-0">
                <button on:click={goBack} class="p-1.5 rounded hover:bg-dark-700 text-dark-400 hover:text-white" title="Back to files">
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" />
                    </svg>
                </button>
                <div class="flex items-center gap-2 text-sm overflow-x-auto">
                    <a href="/containers/{containerId}/files" class="text-dark-400 hover:text-white">
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12l8.954-8.955c.44-.439 1.152-.439 1.591 0L21.75 12M4.5 9.75v10.125c0 .621.504 1.125 1.125 1.125H9.75v-4.875c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125V21h4.125c.621 0 1.125-.504 1.125-1.125V9.75M8.25 21h8.25" />
                        </svg>
                    </a>
                    {#each breadcrumbs as crumb, i}
                        {@const iconType = getFileIcon(crumb.name)}
                        <span class="text-dark-600">/</span>
                        {#if crumb.isFile}
                            <span class="text-white flex items-center gap-1.5">
                                <!-- File icon based on type -->
                                {#if iconType === 'java'}
                                    <svg class="w-4 h-4 text-orange-400" fill="currentColor" viewBox="0 0 24 24">
                                        <path d="M8.851 18.56s-.917.534.653.714c1.902.218 2.874.187 4.969-.211 0 0 .552.346 1.321.646-4.699 2.013-10.633-.118-6.943-1.149M8.276 15.933s-1.028.761.542.924c2.032.209 3.636.227 6.413-.308 0 0 .384.389.987.602-5.679 1.661-12.007.13-7.942-1.218M13.116 11.475c1.158 1.333-.304 2.533-.304 2.533s2.939-1.518 1.589-3.418c-1.261-1.772-2.228-2.652 3.007-5.688 0-.001-8.216 2.051-4.292 6.573"/>
                                    </svg>
                                {:else if iconType === 'code'}
                                    <svg class="w-4 h-4 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M17.25 6.75L22.5 12l-5.25 5.25m-10.5 0L1.5 12l5.25-5.25m7.5-3l-4.5 16.5" />
                                    </svg>
                                {:else if iconType === 'config'}
                                    <svg class="w-4 h-4 text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M10.343 3.94c.09-.542.56-.94 1.11-.94h1.093c.55 0 1.02.398 1.11.94l.149.894c.07.424.384.764.78.93.398.164.855.142 1.205-.108l.737-.527a1.125 1.125 0 011.45.12l.773.774c.39.389.44 1.002.12 1.45l-.527.737c-.25.35-.272.806-.107 1.204.165.397.505.71.93.78l.893.15c.543.09.94.56.94 1.109v1.094c0 .55-.397 1.02-.94 1.11l-.893.149c-.425.07-.765.383-.93.78-.165.398-.143.854.107 1.204l.527.738c.32.447.269 1.06-.12 1.45l-.774.773a1.125 1.125 0 01-1.449.12l-.738-.527c-.35-.25-.806-.272-1.204-.107-.397.165-.71.505-.78.929l-.15.894c-.09.542-.56.94-1.11.94h-1.094c-.55 0-1.019-.398-1.11-.94l-.148-.894c-.071-.424-.384-.764-.781-.93-.398-.164-.854-.142-1.204.108l-.738.527c-.447.32-1.06.269-1.45-.12l-.773-.774a1.125 1.125 0 01-.12-1.45l.527-.737c.25-.35.273-.806.108-1.204-.165-.397-.506-.71-.93-.78l-.894-.15c-.542-.09-.94-.56-.94-1.109v-1.094c0-.55.398-1.02.94-1.11l.894-.149c.424-.07.765-.383.93-.78.165-.398.143-.854-.107-1.204l-.527-.738a1.125 1.125 0 01.12-1.45l.773-.773a1.125 1.125 0 011.45-.12l.737.527c.35.25.807.272 1.204.107.397-.165.71-.505.78-.929l.15-.894z"/>
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/>
                                    </svg>
                                {:else if iconType === 'data'}
                                    <svg class="w-4 h-4 text-yellow-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M17.593 3.322c1.1.128 1.907 1.077 1.907 2.185V21L12 17.25 4.5 21V5.507c0-1.108.806-2.057 1.907-2.185a48.507 48.507 0 0111.186 0z"/>
                                    </svg>
                                {:else}
                                    <svg class="w-4 h-4 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/>
                                    </svg>
                                {/if}
                                <span class="truncate max-w-[200px]">{crumb.name}</span>
                            </span>
                        {:else}
                            <a href="/containers/{containerId}/files?path=/{crumb.path}" class="text-dark-400 hover:text-white truncate max-w-[100px]">{crumb.name}</a>
                        {/if}
                    {/each}
                </div>
            </div>
            <div class="flex items-center gap-2">
                {#if hasChanges}
                    <span class="text-xs text-yellow-400 bg-yellow-400/10 px-2 py-1 rounded flex items-center gap-1">
                        <span class="w-1.5 h-1.5 bg-yellow-400 rounded-full animate-pulse"></span>
                        Unsaved
                    </span>
                {/if}
                <span class="text-xs text-dark-500 bg-dark-800 px-2 py-1 rounded hidden sm:inline">{language}</span>
                <button on:click={loadFile} disabled={loading} class="btn-ghost text-sm p-2" title="Reload file">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99" />
                    </svg>
                </button>
                <button on:click={saveFile} disabled={saving || !hasChanges} class="btn-primary text-sm">
                    {#if saving}
                        <span class="spinner w-4 h-4 mr-1"></span>
                        Saving...
                    {:else}
                        <svg class="w-4 h-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M17 3H7a2 2 0 00-2 2v14a2 2 0 002 2h10a2 2 0 002-2V5a2 2 0 00-2-2z" />
                            <path stroke-linecap="round" stroke-linejoin="round" d="M11 3v4a1 1 0 001 1h4M7 13h10M7 17h7" />
                        </svg>
                        Save
                    {/if}
                </button>
            </div>
        </div>
    </div>

    <!-- Editor -->
    <div class="flex-1 overflow-hidden relative">
        {#if loading}
            <div class="absolute inset-0 flex items-center justify-center bg-dark-950 z-10">
                <div class="text-center">
                    <div class="spinner w-8 h-8 mx-auto mb-3"></div>
                    <p class="text-dark-400 text-sm">Loading file...</p>
                </div>
            </div>
        {/if}
        <div bind:this={editorContainer} class="w-full h-full" class:invisible={loading}></div>
    </div>

    <!-- Status bar -->
    <div class="flex-shrink-0 border-t border-dark-700 bg-dark-800 px-4 py-1.5 text-xs text-dark-400 flex items-center justify-between">
        <div class="flex items-center gap-4">
            <span>Lines: {fileContent.split('\n').length}</span>
            <span>Characters: {fileContent.length.toLocaleString()}</span>
        </div>
        <div class="flex items-center gap-4">
            <span class="hidden sm:inline text-dark-500">{filePath}</span>
            <div class="flex items-center gap-1">
                <kbd class="px-1.5 py-0.5 bg-dark-700 rounded text-dark-300 text-[10px]">⌘S</kbd>
                <span class="text-dark-500">save</span>
            </div>
        </div>
    </div>
</div>

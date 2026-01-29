<script lang="ts">
    import { onMount, onDestroy, createEventDispatcher } from 'svelte';
    import { browser } from '$app/environment';

    export let value: string = '';
    export let language: string = 'plaintext';
    export let readOnly: boolean = false;

    const dispatch = createEventDispatcher();

    let editorContainer: HTMLDivElement;
    let editor: any;
    let monaco: any;

    $: if (editor && value !== editor.getValue()) {
        editor.setValue(value);
    }

    $: if (editor && monaco) {
        const model = editor.getModel();
        if (model) {
            monaco.editor.setModelLanguage(model, mapLanguage(language));
        }
    }

    function mapLanguage(lang: string): string {
        const langMap: Record<string, string> = {
            'js': 'javascript',
            'ts': 'typescript',
            'yml': 'yaml',
            'properties': 'ini',
            'conf': 'ini',
            'bash': 'shell',
            'sh': 'shell',
            'batch': 'bat',
            'plaintext': 'plaintext',
        };
        return langMap[lang] || lang;
    }

    onMount(async () => {
        if (!browser) return;

        // Dynamic import for Monaco Editor
        monaco = await import('monaco-editor');

        // Configure Monaco environment
        self.MonacoEnvironment = {
            getWorker: function (_moduleId: any, label: string) {
                // For simplicity, we'll use the basic editor without workers
                // This reduces bundle size and complexity
                return null as any;
            }
        };

        editor = monaco.editor.create(editorContainer, {
            value: value,
            language: mapLanguage(language),
            theme: 'vs-dark',
            automaticLayout: true,
            minimap: { enabled: false },
            fontSize: 14,
            fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
            lineNumbers: 'on',
            wordWrap: 'off',
            scrollBeyondLastLine: false,
            renderLineHighlight: 'line',
            selectOnLineNumbers: true,
            roundedSelection: true,
            readOnly: readOnly,
            cursorStyle: 'line',
            contextmenu: true,
            scrollbar: {
                vertical: 'auto',
                horizontal: 'auto',
                useShadows: true,
                verticalHasArrows: false,
                horizontalHasArrows: false,
                verticalScrollbarSize: 10,
                horizontalScrollbarSize: 10,
            },
            overviewRulerBorder: false,
            padding: { top: 10, bottom: 10 },
        });

        // Listen for content changes
        editor.onDidChangeModelContent(() => {
            const newValue = editor.getValue();
            if (newValue !== value) {
                value = newValue;
                dispatch('change', { value: newValue });
            }
        });

        // Add keyboard shortcuts
        editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
            dispatch('save');
        });
    });

    onDestroy(() => {
        if (editor) {
            editor.dispose();
        }
    });
</script>

<div bind:this={editorContainer} class="w-full h-full"></div>

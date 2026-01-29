<script lang="ts">
    import { getContext } from 'svelte';
    import type { Writable } from 'svelte/store';
    import type { Container } from '$lib/types';

    const containerStore = getContext<Writable<Container | null>>('container');
    const logsStore = getContext<Writable<string[]>>('logs');
    const actions = getContext<any>('actions');

    let logsContainer: HTMLDivElement;
    let command = '';
    let userScrolled = false;
    let lastScrollTop = 0;

    $: container = $containerStore;
    $: logs = $logsStore;
    $: isRunning = container?.status?.toLowerCase() === 'running';

    // Auto-scroll logs only if user hasn't scrolled up
    $: if (logs && logsContainer && !userScrolled) {
        setTimeout(() => {
            logsContainer.scrollTop = logsContainer.scrollHeight;
        }, 10);
    }

    function handleScroll() {
        if (!logsContainer) return;
        const isAtBottom = logsContainer.scrollHeight - logsContainer.scrollTop <= logsContainer.clientHeight + 50;
        // Detect if user is scrolling up
        if (logsContainer.scrollTop < lastScrollTop && !isAtBottom) {
            userScrolled = true;
        }
        // Reset if user scrolls to bottom
        if (isAtBottom) {
            userScrolled = false;
        }
        lastScrollTop = logsContainer.scrollTop;
    }

    function handleKeydown(event: KeyboardEvent) {
        if (event.key === 'Enter' && command.trim()) {
            actions.sendCommand(command);
            command = '';
        }
    }

    function formatAnsiColors(text: string): string {
        const colors: Record<string, string> = {
            '0': '</span>',
            '31': '<span class="text-red-400">',
            '32': '<span class="text-emerald-400">',
            '33': '<span class="text-yellow-400">',
            '34': '<span class="text-blue-400">',
            '35': '<span class="text-purple-400">',
            '36': '<span class="text-cyan-400">',
        };
        return text.replace(/\x1b\[(\d+)m/g, (_, code) => colors[code] || '');
    }
</script>

<div class="h-full flex flex-col">
    <div bind:this={logsContainer} on:scroll={handleScroll} class="flex-1 overflow-y-auto p-4 font-mono text-sm bg-dark-950">
        {#each logs as log}
            <div class="whitespace-pre-wrap break-all">{@html formatAnsiColors(log)}</div>
        {/each}
        {#if logs.length === 0}
            <div class="text-dark-500 text-center py-8">No logs yet. Start the server to see console output.</div>
        {/if}
    </div>
    <div class="flex-shrink-0 p-4 border-t border-dark-700 bg-dark-900">
        {#if isRunning}
            <div class="flex gap-2">
                <span class="text-primary-400 font-mono">$</span>
                <input
                    type="text"
                    bind:value={command}
                    on:keydown={handleKeydown}
                    placeholder="Type a command..."
                    class="flex-1 bg-transparent text-white font-mono text-sm focus:outline-none placeholder-dark-500"
                />
            </div>
        {:else}
            <div class="text-center text-dark-500 text-sm py-1">
                Container is not running. Start the server to use the console.
            </div>
        {/if}
    </div>
</div>

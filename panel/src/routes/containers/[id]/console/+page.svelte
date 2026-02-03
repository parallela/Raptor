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
    let inputFocused = false;

    $: container = $containerStore;
    $: logs = $logsStore;
    $: isRunning = container?.status?.toLowerCase() === 'running';

    $: if (logs && logsContainer && !userScrolled) {
        setTimeout(() => {
            logsContainer.scrollTop = logsContainer.scrollHeight;
        }, 10);
    }

    function handleScroll() {
        if (!logsContainer) return;
        const isAtBottom = logsContainer.scrollHeight - logsContainer.scrollTop <= logsContainer.clientHeight + 50;
        if (logsContainer.scrollTop < lastScrollTop && !isAtBottom) {
            userScrolled = true;
        }
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

    function scrollToBottom() {
        if (logsContainer) {
            logsContainer.scrollTop = logsContainer.scrollHeight;
            userScrolled = false;
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

<svelte:head>
    <title>{container?.name || 'Container'} - Console - Raptor</title>
</svelte:head>

<div class="h-full flex flex-col relative">
    <div bind:this={logsContainer} on:scroll={handleScroll} class="flex-1 overflow-y-auto p-2 md:p-4 font-mono text-xs md:text-sm bg-dark-950">
        {#each logs as log}
            <div class="whitespace-pre-wrap break-all leading-relaxed">{@html formatAnsiColors(log)}</div>
        {/each}
        {#if logs.length === 0}
            <div class="text-dark-500 text-center py-8">No logs yet. Start the server to see console output.</div>
        {/if}
    </div>

    <!-- Scroll to bottom button -->
    {#if userScrolled}
        <button
            on:click={scrollToBottom}
            class="absolute bottom-20 right-4 md:bottom-24 md:right-6 p-2 bg-primary-500 hover:bg-primary-400 text-white rounded-full shadow-lg transition-all"
            title="Scroll to bottom"
        >
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M19 14l-7 7m0 0l-7-7m7 7V3" />
            </svg>
        </button>
    {/if}

    <div class="flex-shrink-0 p-2 md:p-4 border-t border-dark-700 bg-dark-900">
        {#if isRunning}
            <div class="flex gap-2 items-center">
                <span class="text-primary-400 font-mono text-sm md:text-base">$</span>
                <input
                    type="text"
                    bind:value={command}
                    on:keydown={handleKeydown}
                    on:focus={() => inputFocused = true}
                    on:blur={() => inputFocused = false}
                    placeholder="Type a command..."
                    class="flex-1 bg-transparent text-white font-mono text-sm md:text-base focus:outline-none placeholder-dark-500"
                />
                <!-- Mobile send button -->
                <button
                    on:click={() => { if (command.trim()) { actions.sendCommand(command); command = ''; } }}
                    class="md:hidden p-2 text-primary-400 hover:text-primary-300 disabled:opacity-50"
                    disabled={!command.trim()}
                >
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M13 5l7 7-7 7M5 5l7 7-7 7" />
                    </svg>
                </button>
            </div>
        {:else}
            <div class="text-center text-dark-500 text-xs md:text-sm py-1">
                Container is not running. Start the server to use the console.
            </div>
        {/if}
    </div>
</div>

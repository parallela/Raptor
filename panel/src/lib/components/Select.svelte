<script lang="ts">
    import { createEventDispatcher, onMount } from 'svelte';

    export let value: string = '';
    export let placeholder: string = 'Select an option...';
    export let options: { value: string; label: string }[] = [];
    export let disabled: boolean = false;
    export let required: boolean = false;
    export let id: string = '';

    const dispatch = createEventDispatcher();

    let open = false;
    let container: HTMLDivElement;
    let searchQuery = '';
    let highlightedIndex = -1;

    $: selectedOption = options.find(opt => opt.value === value);
    $: filteredOptions = searchQuery
        ? options.filter(opt => opt.label.toLowerCase().includes(searchQuery.toLowerCase()))
        : options;

    function toggleOpen() {
        if (disabled) return;
        open = !open;
        if (open) {
            searchQuery = '';
            highlightedIndex = options.findIndex(opt => opt.value === value);
        }
    }

    function selectOption(option: { value: string; label: string }) {
        value = option.value;
        open = false;
        searchQuery = '';
        dispatch('change', option.value);
    }

    function handleKeydown(e: KeyboardEvent) {
        if (!open) {
            if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown') {
                e.preventDefault();
                open = true;
            }
            return;
        }

        switch (e.key) {
            case 'Escape':
                open = false;
                searchQuery = '';
                break;
            case 'ArrowDown':
                e.preventDefault();
                highlightedIndex = Math.min(highlightedIndex + 1, filteredOptions.length - 1);
                break;
            case 'ArrowUp':
                e.preventDefault();
                highlightedIndex = Math.max(highlightedIndex - 1, 0);
                break;
            case 'Enter':
                e.preventDefault();
                if (highlightedIndex >= 0 && filteredOptions[highlightedIndex]) {
                    selectOption(filteredOptions[highlightedIndex]);
                }
                break;
        }
    }

    function handleClickOutside(e: MouseEvent) {
        if (container && !container.contains(e.target as Node)) {
            open = false;
            searchQuery = '';
        }
    }

    onMount(() => {
        document.addEventListener('click', handleClickOutside);
        return () => document.removeEventListener('click', handleClickOutside);
    });
</script>

<div class="relative" bind:this={container}>
    <!-- Hidden input for form submission -->
    <input type="hidden" {id} {value} {required} />

    <!-- Trigger button -->
    <button
        type="button"
        on:click={toggleOpen}
        on:keydown={handleKeydown}
        class="w-full px-4 py-2.5 bg-dark-800/50 border rounded-lg text-left transition-all duration-200 focus:outline-none flex items-center justify-between gap-2
            {disabled ? 'opacity-50 cursor-not-allowed border-dark-600/50' : 'border-dark-600/50 hover:border-dark-500/50 hover:bg-dark-800/70'}
            {open ? 'border-primary-500/50 ring-2 ring-primary-500/20 bg-dark-800' : ''}"
        {disabled}
    >
        <span class="truncate {selectedOption ? 'text-dark-100' : 'text-dark-400'}">
            {selectedOption?.label || placeholder}
        </span>
        <svg
            class="w-5 h-5 text-dark-400 transition-transform duration-200 flex-shrink-0 {open ? 'rotate-180' : ''}"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width="2"
        >
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
        </svg>
    </button>

    <!-- Dropdown -->
    {#if open}
        <div class="absolute z-50 w-full mt-2 bg-dark-800 border border-dark-600/50 rounded-lg shadow-xl shadow-black/20 overflow-hidden animate-slide-down">
            <!-- Search input for many options -->
            {#if options.length > 5}
                <div class="flex items-center gap-2 px-3 py-2 border-b border-dark-700/50">
                    <svg class="w-4 h-4 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                    </svg>
                    <input
                        type="text"
                        bind:value={searchQuery}
                        placeholder="Search..."
                        class="flex-1 bg-transparent text-sm text-dark-100 placeholder-dark-400 focus:outline-none"
                        on:keydown={handleKeydown}
                    />
                </div>
            {/if}

            <!-- Options list -->
            <div class="max-h-60 overflow-y-auto py-1">
                {#if filteredOptions.length === 0}
                    <div class="px-4 py-3 text-sm text-dark-400 text-center">No options found</div>
                {:else}
                    {#each filteredOptions as option, i}
                        <button
                            type="button"
                            class="w-full px-4 py-2.5 text-left text-sm transition-colors duration-100 flex items-center justify-between gap-2
                                {option.value === value ? 'text-primary-400 bg-primary-500/10' : 'text-dark-200'}
                                {i === highlightedIndex ? 'bg-dark-700/50 text-white' : ''}
                                {option.value === value && i === highlightedIndex ? 'bg-primary-500/20' : ''}"
                            on:click={() => selectOption(option)}
                            on:mouseenter={() => highlightedIndex = i}
                        >
                            <span>{option.label}</span>
                            {#if option.value === value}
                                <svg class="w-4 h-4 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                                </svg>
                            {/if}
                        </button>
                    {/each}
                {/if}
            </div>
        </div>
    {/if}
</div>

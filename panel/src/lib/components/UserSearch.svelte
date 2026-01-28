<script lang="ts">
    import { createEventDispatcher } from 'svelte';
    import { api } from '$lib/api';
    import type { User } from '$lib/types';

    export let value: string = '';
    export let placeholder: string = 'Search users...';
    export let disabled: boolean = false;

    const dispatch = createEventDispatcher<{ select: User }>();

    let searchQuery = '';
    let results: User[] = [];
    let loading = false;
    let showDropdown = false;
    let searchTimeout: ReturnType<typeof setTimeout>;
    let inputElement: HTMLInputElement;

    async function search(query: string) {
        if (!query.trim()) {
            results = [];
            return;
        }

        loading = true;
        try {
            results = await api.searchUsers(query, 10);
        } catch (e) {
            console.error('Search failed:', e);
            results = [];
        } finally {
            loading = false;
        }
    }

    function handleInput() {
        clearTimeout(searchTimeout);
        searchTimeout = setTimeout(() => {
            search(searchQuery);
        }, 300);
        showDropdown = true;
    }

    function selectUser(user: User) {
        value = user.id;
        searchQuery = user.username;
        showDropdown = false;
        results = [];
        dispatch('select', user);
    }

    function handleFocus() {
        if (searchQuery.trim()) {
            showDropdown = true;
        }
    }

    function handleBlur() {
        // Delay to allow click on result
        setTimeout(() => {
            showDropdown = false;
        }, 200);
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') {
            showDropdown = false;
        }
    }

    export function clear() {
        value = '';
        searchQuery = '';
        results = [];
        showDropdown = false;
    }
</script>

<div class="relative">
    <div class="relative">
        <input
            type="text"
            bind:this={inputElement}
            bind:value={searchQuery}
            on:input={handleInput}
            on:focus={handleFocus}
            on:blur={handleBlur}
            on:keydown={handleKeydown}
            {placeholder}
            {disabled}
            class="input w-full pr-10"
            autocomplete="off"
        />
        <div class="absolute right-3 top-1/2 -translate-y-1/2">
            {#if loading}
                <span class="spinner w-4 h-4"></span>
            {:else}
                <svg class="w-4 h-4 text-dark-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
                </svg>
            {/if}
        </div>
    </div>

    {#if showDropdown && (results.length > 0 || loading)}
        <div class="absolute z-50 w-full mt-1 bg-dark-800 border border-dark-700 rounded-lg shadow-xl max-h-60 overflow-y-auto">
            {#if results.length > 0}
                {#each results as user}
                    <button
                        type="button"
                        on:click={() => selectUser(user)}
                        class="w-full px-4 py-3 flex items-center gap-3 hover:bg-dark-700 transition-colors text-left"
                    >
                        <div class="w-8 h-8 rounded-full bg-gradient-to-br from-primary-400 to-primary-600 flex items-center justify-center text-white font-semibold text-sm flex-shrink-0">
                            {user.username.charAt(0).toUpperCase()}
                        </div>
                        <div class="min-w-0 flex-1">
                            <p class="text-white font-medium truncate">{user.username}</p>
                            {#if user.email}
                                <p class="text-dark-400 text-xs truncate">{user.email}</p>
                            {/if}
                        </div>
                        {#if user.roleName}
                            <span class="text-xs px-2 py-0.5 rounded bg-dark-700 text-dark-400">{user.roleName}</span>
                        {/if}
                    </button>
                {/each}
            {:else if loading}
                <div class="px-4 py-3 text-center">
                    <span class="spinner w-5 h-5 mx-auto"></span>
                </div>
            {/if}
        </div>
    {/if}

    {#if searchQuery && !showDropdown && results.length === 0 && !loading}
        <!-- No results message could go here -->
    {/if}
</div>

<script lang="ts">
    import { onMount } from 'svelte';
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { api } from '$lib/api';
    import toast from 'svelte-french-toast';
    import { _ } from '$lib/i18n';
    import { LocaleSelector } from '$lib/components';

    let token = '';
    let username = '';
    let password = '';
    let confirmPassword = '';
    let loading = false;
    let validating = true;
    let invalidToken = false;

    onMount(() => {
        token = $page.url.searchParams.get('token') || '';
        if (!token) {
            invalidToken = true;
        }
        validating = false;
    });

    async function acceptInvite() {
        if (!username.trim()) {
            toast.error('Please enter a username');
            return;
        }
        if (password.length < 8) {
            toast.error('Password must be at least 8 characters');
            return;
        }
        if (password !== confirmPassword) {
            toast.error('Passwords do not match');
            return;
        }

        loading = true;
        try {
            await api.acceptInvite(token, username.trim(), password);
            toast.success('Account created successfully! You can now log in.');
            goto('/login');
        } catch (e: any) {
            toast.error(e.message || 'Failed to accept invitation');
        } finally {
            loading = false;
        }
    }
</script>

<svelte:head>
    <title>{$_('invite.title')} - Raptor</title>
</svelte:head>

<div class="min-h-screen flex items-center justify-center p-4 relative">
    <!-- Language Selector - Top Right -->
    <div class="absolute top-4 right-4 z-10">
        <LocaleSelector />
    </div>

    <div class="w-full max-w-md">
        <div class="text-center mb-8">
            <img src="/logo.webp" alt="Raptor" class="h-12 mx-auto mb-4" />
            <h1 class="text-2xl font-bold text-white">{$_('invite.title')}</h1>
            <p class="text-dark-400 mt-2">{$_('invite.subtitle')}</p>
        </div>

        {#if validating}
            <div class="card p-8 text-center">
                <span class="spinner w-8 h-8 mx-auto"></span>
                <p class="text-dark-400 mt-4">{$_('common.loading')}</p>
            </div>
        {:else if invalidToken}
            <div class="card p-8 text-center">
                <div class="w-16 h-16 rounded-full bg-red-500/20 flex items-center justify-center mx-auto mb-4">
                    <svg class="w-8 h-8 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </div>
                <h2 class="text-xl font-semibold text-white mb-2">{$_('invite.invalidToken')}</h2>
                <p class="text-dark-400 mb-6">{$_('invite.invalidToken')}</p>
                <a href="/login" class="btn-primary inline-flex">{$_('auth.login')}</a>
            </div>
        {:else}
            <form on:submit|preventDefault={acceptInvite} class="card p-8 space-y-6">
                <div class="input-group">
                    <label for="username" class="input-label">{$_('auth.username')}</label>
                    <input
                        type="text"
                        id="username"
                        bind:value={username}
                        class="input"
                        placeholder={$_('auth.enterUsername')}
                        required
                        autocomplete="username"
                    />
                </div>

                <div class="input-group">
                    <label for="password" class="input-label">{$_('auth.password')}</label>
                    <input
                        type="password"
                        id="password"
                        bind:value={password}
                        class="input"
                        placeholder={$_('auth.enterPassword')}
                        required
                        autocomplete="new-password"
                        minlength="8"
                    />
                    <p class="text-xs text-dark-500 mt-1">{$_('validation.minLength', { min: '8' })}</p>
                </div>

                <div class="input-group">
                    <label for="confirm-password" class="input-label">{$_('auth.confirmPassword')}</label>
                    <input
                        type="password"
                        id="confirm-password"
                        bind:value={confirmPassword}
                        class="input"
                        placeholder={$_('auth.enterPassword')}
                        required
                        autocomplete="new-password"
                    />
                </div>

                <button type="submit" class="btn-primary w-full" disabled={loading}>
                    {#if loading}
                        <span class="spinner w-5 h-5 mr-2"></span>
                    {/if}
                    {$_('invite.completeSetup')}
                </button>

                <p class="text-center text-dark-400 text-sm">
                    {$_('auth.alreadyHaveAccount')} <a href="/login" class="text-primary-400 hover:text-primary-300">{$_('auth.signIn')}</a>
                </p>
            </form>
        {/if}
    </div>
</div>

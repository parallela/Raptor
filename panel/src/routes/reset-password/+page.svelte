<script lang="ts">
    import { api } from '$lib/api';
    import { goto } from '$app/navigation';
    import { page } from '$app/stores';
    import { _ } from '$lib/i18n';
    import { LocaleSelector } from '$lib/components';

    let password = '';
    let confirmPassword = '';
    let error = '';
    let success = '';
    let loading = false;

    $: token = $page.url.searchParams.get('token') || '';

    async function handleSubmit() {
        error = '';
        success = '';

        if (password !== confirmPassword) {
            error = 'Passwords do not match';
            return;
        }

        if (password.length < 6) {
            error = 'Password must be at least 6 characters';
            return;
        }

        loading = true;
        try {
            await api.resetPassword(token, password);
            success = 'Password has been reset successfully! Redirecting to login...';
            setTimeout(() => goto('/login'), 2000);
        } catch (e: any) {
            error = e.message || 'An error occurred';
        } finally {
            loading = false;
        }
    }
</script>

<svelte:head>
    <title>{$_('auth.resetPassword')} - Raptor</title>
</svelte:head>

<div class="min-h-screen flex items-center justify-center p-4 relative overflow-hidden">
    <!-- Animated background -->
    <div class="absolute inset-0 overflow-hidden">
        <div class="absolute top-1/4 -left-20 w-96 h-96 bg-primary-500/20 rounded-full blur-3xl animate-pulse-slow"></div>
        <div class="absolute bottom-1/4 -right-20 w-96 h-96 bg-primary-600/10 rounded-full blur-3xl animate-pulse-slow" style="animation-delay: 1s;"></div>
        <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] bg-primary-500/5 rounded-full blur-3xl"></div>
    </div>

    <!-- Language Selector - Top Right -->
    <div class="absolute top-4 right-4 z-10">
        <LocaleSelector />
    </div>

    <div class="relative w-full max-w-md animate-slide-up">
        <!-- Logo -->
        <div class="text-center mb-8">
            <div class="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-dark-800/50 shadow-xl shadow-primary-500/20 mb-4 p-2">
                <img src="/logo.webp" alt="Raptor Logo" class="w-full h-full object-contain" />
            </div>
            <h1 class="text-3xl font-bold text-white">Raptor</h1>
            <p class="text-dark-400 mt-2">Container Management Panel</p>
        </div>

        <!-- Card -->
        <div class="card p-8">
            <h2 class="text-xl font-semibold text-white text-center mb-6">
                {$_('auth.resetYourPassword')}
            </h2>

            {#if !token}
                <div class="flex items-center gap-3 p-4 rounded-lg bg-red-500/10 border border-red-500/20 mb-6">
                    <svg class="w-5 h-5 text-red-400 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
                    </svg>
                    <span class="text-red-400 text-sm">{$_('invite.invalidToken')}</span>
                </div>
                <a href="/login" class="btn-primary w-full h-12 text-base flex items-center justify-center">
                    {$_('auth.backToLogin')}
                </a>
            {:else}
                {#if error}
                    <div class="flex items-center gap-3 p-4 rounded-lg bg-red-500/10 border border-red-500/20 mb-6 animate-slide-down">
                        <svg class="w-5 h-5 text-red-400 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
                        </svg>
                        <span class="text-red-400 text-sm">{error}</span>
                    </div>
                {/if}

                {#if success}
                    <div class="flex items-center gap-3 p-4 rounded-lg bg-emerald-500/10 border border-emerald-500/20 mb-6 animate-slide-down">
                        <svg class="w-5 h-5 text-emerald-400 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        <span class="text-emerald-400 text-sm">{success}</span>
                    </div>
                {:else}
                    <form on:submit|preventDefault={handleSubmit} class="space-y-5">
                        <div class="input-group">
                            <label for="password" class="input-label">{$_('auth.newPassword')}</label>
                            <div class="relative">
                                <div class="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none">
                                    <svg class="w-5 h-5 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M16.5 10.5V6.75a4.5 4.5 0 10-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 002.25-2.25v-6.75a2.25 2.25 0 00-2.25-2.25H6.75a2.25 2.25 0 00-2.25 2.25v6.75a2.25 2.25 0 002.25 2.25z" />
                                    </svg>
                                </div>
                                <input
                                    type="password"
                                    id="password"
                                    bind:value={password}
                                    class="input pl-12"
                                    placeholder={$_('auth.enterPassword')}
                                    required
                                    minlength="6"
                                />
                            </div>
                        </div>

                        <div class="input-group">
                            <label for="confirmPassword" class="input-label">{$_('auth.confirmPassword')}</label>
                            <div class="relative">
                                <div class="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none">
                                    <svg class="w-5 h-5 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M16.5 10.5V6.75a4.5 4.5 0 10-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 002.25-2.25v-6.75a2.25 2.25 0 00-2.25-2.25H6.75a2.25 2.25 0 00-2.25 2.25v6.75a2.25 2.25 0 002.25 2.25z" />
                                    </svg>
                                </div>
                                <input
                                    type="password"
                                    id="confirmPassword"
                                    bind:value={confirmPassword}
                                    class="input pl-12"
                                    placeholder={$_('auth.enterPassword')}
                                    required
                                />
                            </div>
                        </div>

                        <button
                            type="submit"
                            class="btn-primary w-full h-12 text-base"
                            disabled={loading}
                        >
                            {#if loading}
                                <span class="spinner"></span>
                                <span>{$_('common.loading')}</span>
                            {:else}
                                {$_('auth.resetPassword')}
                            {/if}
                        </button>
                    </form>
                {/if}
            {/if}

            <div class="mt-6 pt-6 border-t border-dark-700/50">
                <p class="text-center text-dark-400 text-sm">
                    <a href="/login" class="text-primary-400 hover:text-primary-300 font-medium transition-colors duration-200">
                        {$_('auth.backToLogin')}
                    </a>
                </p>
            </div>
        </div>

        <!-- Footer -->
        <p class="text-center text-dark-500 text-xs mt-8">
            Powered by Raptor • Secure Container Management
        </p>
    </div>
</div>

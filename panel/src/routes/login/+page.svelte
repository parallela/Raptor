<script lang="ts">
    import { api } from '$lib/api';
    import { user, token } from '$lib/stores';
    import { goto } from '$app/navigation';
    import { _ } from '$lib/i18n';
    import { LocaleSelector } from '$lib/components';

    let username = '';
    let email = '';
    let password = '';
    let error = '';
    let success = '';
    let loading = false;
    let isRegister = false;
    let isForgotPassword = false;

    async function handleSubmit() {
        error = '';
        success = '';
        loading = true;
        try {
            if (isForgotPassword) {
                await api.forgotPassword(email);
                success = 'If an account with that email exists, a password reset link has been sent.';
                isForgotPassword = false;
                email = '';
            } else if (isRegister) {
                await api.register(username, email, password);
                isRegister = false;
                success = 'Account created successfully! Please login.';
            } else {
                const res = await api.login(username, password);
                $token = res.token;
                $user = res.user;
                goto('/');
            }
        } catch (e: any) {
            error = e.message || 'An error occurred';
        } finally {
            loading = false;
        }
    }

    function resetForm() {
        error = '';
        success = '';
        username = '';
        email = '';
        password = '';
    }
</script>

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
            <div class="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-dark-700/50 shadow-xl shadow-primary-500/20 mb-4">
                <img src="/favicon.png" alt="Raptor Logo" class="w-14 h-14 object-contain" />
            </div>
            <h1 class="text-2xl font-bold text-white">Raptor</h1>
            <p class="text-dark-400 mt-2">Container Management Panel</p>
        </div>

        <!-- Card -->
        <div class="card p-8">
            <h2 class="text-xl font-semibold text-white text-center mb-6">
                {#if isForgotPassword}
                    {$_('auth.resetYourPassword')}
                {:else if isRegister}
                    {$_('auth.createAccount')}
                {:else}
                    {$_('auth.welcomeBack')}
                {/if}
            </h2>

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
            {/if}

            <form on:submit|preventDefault={handleSubmit} class="space-y-5">
                {#if isForgotPassword}
                    <!-- Forgot Password: Email only -->
                    <div class="input-group">
                        <label for="email" class="input-label">{$_('auth.email')}</label>
                        <div class="relative">
                            <div class="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none">
                                <svg class="w-5 h-5 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M21.75 6.75v10.5a2.25 2.25 0 01-2.25 2.25h-15a2.25 2.25 0 01-2.25-2.25V6.75m19.5 0A2.25 2.25 0 0019.5 4.5h-15a2.25 2.25 0 00-2.25 2.25m19.5 0v.243a2.25 2.25 0 01-1.07 1.916l-7.5 4.615a2.25 2.25 0 01-2.36 0L3.32 8.91a2.25 2.25 0 01-1.07-1.916V6.75" />
                                </svg>
                            </div>
                            <input
                                type="email"
                                id="email"
                                bind:value={email}
                                class="input pl-12"
                                placeholder={$_('auth.enterEmail')}
                                required
                            />
                        </div>
                    </div>
                {:else}
                    <!-- Username field -->
                    <div class="input-group">
                        <label for="username" class="input-label">{isRegister ? $_('auth.username') : $_('auth.usernameOrEmail')}</label>
                        <div class="relative">
                            <div class="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none">
                                <svg class="w-5 h-5 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 6a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0zM4.501 20.118a7.5 7.5 0 0114.998 0A17.933 17.933 0 0112 21.75c-2.676 0-5.216-.584-7.499-1.632z" />
                                </svg>
                            </div>
                            <input
                                type="text"
                                id="username"
                                bind:value={username}
                                class="input pl-12"
                                placeholder={$_('auth.enterUsername')}
                                required
                            />
                        </div>
                    </div>

                    <!-- Email field (only for registration) -->
                    {#if isRegister}
                        <div class="input-group">
                            <label for="email" class="input-label">{$_('auth.email')}</label>
                            <div class="relative">
                                <div class="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none">
                                    <svg class="w-5 h-5 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M21.75 6.75v10.5a2.25 2.25 0 01-2.25 2.25h-15a2.25 2.25 0 01-2.25-2.25V6.75m19.5 0A2.25 2.25 0 0019.5 4.5h-15a2.25 2.25 0 00-2.25 2.25m19.5 0v.243a2.25 2.25 0 01-1.07 1.916l-7.5 4.615a2.25 2.25 0 01-2.36 0L3.32 8.91a2.25 2.25 0 01-1.07-1.916V6.75" />
                                    </svg>
                                </div>
                                <input
                                    type="email"
                                    id="email"
                                    bind:value={email}
                                    class="input pl-12"
                                    placeholder={$_('auth.enterEmail')}
                                    required
                                />
                            </div>
                        </div>
                    {/if}

                    <!-- Password field -->
                    <div class="input-group">
                        <label for="password" class="input-label">{$_('auth.password')}</label>
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
                            />
                        </div>
                    </div>

                    <!-- Forgot password link (only for login) -->
                    {#if !isRegister}
                        <div class="text-right">
                            <button
                                type="button"
                                on:click={() => { isForgotPassword = true; resetForm(); }}
                                class="text-sm text-primary-400 hover:text-primary-300 transition-colors duration-200"
                            >
                                {$_('auth.forgotPassword')}
                            </button>
                        </div>
                    {/if}
                {/if}

                <button
                    type="submit"
                    class="btn-primary w-full h-12 text-base"
                    disabled={loading}
                >
                    {#if loading}
                        <span class="spinner"></span>
                        <span>{$_('common.loading')}</span>
                    {:else if isForgotPassword}
                        {$_('auth.sendResetLink')}
                    {:else if isRegister}
                        {$_('auth.signUp')}
                    {:else}
                        {$_('auth.signIn')}
                    {/if}
                </button>
            </form>

            <div class="mt-6 pt-6 border-t border-dark-700/50">
                <p class="text-center text-dark-400 text-sm">
                    {#if isForgotPassword}
                        {$_('auth.alreadyHaveAccount')}
                        <button
                            on:click={() => { isForgotPassword = false; resetForm(); }}
                            class="text-primary-400 hover:text-primary-300 font-medium ml-1 transition-colors duration-200"
                        >
                            {$_('auth.signIn')}
                        </button>
                    {:else if isRegister}
                        {$_('auth.alreadyHaveAccount')}
                        <button
                            on:click={() => { isRegister = false; resetForm(); }}
                            class="text-primary-400 hover:text-primary-300 font-medium ml-1 transition-colors duration-200"
                        >
                            {$_('auth.signIn')}
                        </button>
                    {:else}
                        {$_('auth.dontHaveAccount')}
                        <button
                            on:click={() => { isRegister = true; resetForm(); }}
                            class="text-primary-400 hover:text-primary-300 font-medium ml-1 transition-colors duration-200"
                        >
                            {$_('auth.signUp')}
                        </button>
                    {/if}
                </p>
            </div>
        </div>

        <!-- Footer -->
        <p class="text-center text-dark-500 text-xs mt-8">
            Powered by Raptor • Secure Container Management
        </p>
    </div>
</div>

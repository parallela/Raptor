<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '$lib/api';
    import { user } from '$lib/stores';
    import { goto } from '$app/navigation';
    import toast from 'svelte-french-toast';
    import { _ } from '$lib/i18n';

    interface TwoFactorStatus {
        enabled: boolean;
        verifiedAt?: string;
    }

    let loading = true;
    let twoFactorStatus: TwoFactorStatus | null = null;

    // Setup state
    let showSetupModal = false;
    let setupStep: 'password' | 'qrcode' | 'verify' | 'backup' = 'password';
    let setupPassword = '';
    let setupCode = '';
    let setupLoading = false;
    let qrCode = '';
    let secret = '';
    let otpauthUrl = '';
    let backupCodes: string[] = [];

    // Disable state
    let showDisableModal = false;
    let disablePassword = '';
    let disableCode = '';
    let disableLoading = false;

    // Regenerate backup codes state
    let showRegenerateModal = false;
    let regenerateCode = '';
    let regenerateLoading = false;
    let newBackupCodes: string[] = [];

    onMount(async () => {
        if (!$user) {
            goto('/login');
            return;
        }
        await loadStatus();
    });

    async function loadStatus() {
        loading = true;
        try {
            twoFactorStatus = await api.get2FAStatus();
        } catch (e: any) {
            toast.error(e.message || 'Failed to load 2FA status');
        } finally {
            loading = false;
        }
    }

    async function startSetup() {
        setupLoading = true;
        try {
            const response = await api.setup2FA(setupPassword);
            qrCode = response.qrCode;
            secret = response.secret;
            otpauthUrl = response.otpauthUrl;
            setupStep = 'qrcode';
        } catch (e: any) {
            toast.error(e.message || 'Failed to setup 2FA');
        } finally {
            setupLoading = false;
        }
    }

    async function verifySetup() {
        setupLoading = true;
        try {
            // Clean the code - remove spaces and ensure it's trimmed
            const cleanCode = setupCode.trim().replace(/\s/g, '').replace(/-/g, '');
            console.log('Verifying 2FA code:', cleanCode, 'length:', cleanCode.length);

            if (cleanCode.length !== 6) {
                toast.error('Please enter a 6-digit code');
                setupLoading = false;
                return;
            }

            const response = await api.verify2FA(cleanCode);
            console.log('Verify 2FA response:', response);

            if (response.success) {
                backupCodes = response.backupCodes || [];
                setupStep = 'backup';
                await loadStatus();
                toast.success('Two-factor authentication enabled!');
            } else {
                toast.error('Invalid verification code. Please try again.');
            }
        } catch (e: any) {
            console.error('Verify 2FA error:', e);
            toast.error(e.message || 'Failed to verify code');
        } finally {
            setupLoading = false;
        }
    }

    async function disable2FA() {
        disableLoading = true;
        try {
            await api.disable2FA(disablePassword, disableCode);
            toast.success('Two-factor authentication disabled');
            showDisableModal = false;
            disablePassword = '';
            disableCode = '';
            await loadStatus();
        } catch (e: any) {
            toast.error(e.message || 'Failed to disable 2FA');
        } finally {
            disableLoading = false;
        }
    }

    async function regenerateBackupCodes() {
        regenerateLoading = true;
        try {
            newBackupCodes = await api.regenerateBackupCodes(regenerateCode);
            toast.success('Backup codes regenerated');
            regenerateCode = '';
        } catch (e: any) {
            toast.error(e.message || 'Failed to regenerate backup codes');
        } finally {
            regenerateLoading = false;
        }
    }

    function closeSetupModal() {
        showSetupModal = false;
        setupStep = 'password';
        setupPassword = '';
        setupCode = '';
        qrCode = '';
        secret = '';
        backupCodes = [];
    }

    function copyBackupCodes() {
        const codes = (backupCodes.length > 0 ? backupCodes : newBackupCodes).join('\n');
        navigator.clipboard.writeText(codes);
        toast.success('Backup codes copied to clipboard');
    }

    function downloadBackupCodes() {
        const codes = (backupCodes.length > 0 ? backupCodes : newBackupCodes).join('\n');
        const blob = new Blob([`Raptor Panel - 2FA Backup Codes\n\nKeep these codes safe. Each code can only be used once.\n\n${codes}`], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'raptor-2fa-backup-codes.txt';
        a.click();
        URL.revokeObjectURL(url);
    }
</script>

<svelte:head>
    <title>{$_('security.title')} - Raptor</title>
</svelte:head>

<div class="space-y-6 max-w-4xl mx-auto">
    <!-- Header -->
    <div class="flex items-center gap-4">
        <a href="/" class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-800 transition-colors">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" />
            </svg>
        </a>
        <div>
            <h1 class="text-2xl font-bold text-white">{$_('security.title')}</h1>
            <p class="text-dark-400 mt-1">{$_('security.subtitle')}</p>
        </div>
    </div>

    {#if loading}
        <div class="card p-12 text-center">
            <div class="spinner w-8 h-8 mx-auto mb-4"></div>
            <p class="text-dark-400">{$_('common.loading')}</p>
        </div>
    {:else}
        <!-- User Info Card -->
        <div class="card p-6">
            <div class="flex items-center gap-4">
                <div class="w-16 h-16 rounded-full bg-primary-500/20 flex items-center justify-center">
                    {#if $user?.avatarUrl}
                        <img src={$user.avatarUrl} alt="Avatar" class="w-16 h-16 rounded-full" />
                    {:else}
                        <span class="text-2xl font-bold text-primary-400">{$user?.username?.charAt(0).toUpperCase()}</span>
                    {/if}
                </div>
                <div>
                    <h2 class="text-xl font-semibold text-white">{$user?.username}</h2>
                    <p class="text-dark-400">{$user?.email || 'No email set'}</p>
                    {#if $user?.roleName}
                        <span class="inline-flex items-center px-2 py-0.5 mt-1 text-xs font-medium rounded-full bg-primary-500/20 text-primary-400">
                            {$user.roleName}
                        </span>
                    {/if}
                </div>
            </div>
        </div>

        <!-- Two-Factor Authentication Card -->
        <div class="card p-6">
            <div class="flex items-start justify-between">
                <div class="flex items-start gap-4">
                    <div class="w-12 h-12 rounded-xl bg-primary-500/10 flex items-center justify-center flex-shrink-0">
                        <svg class="w-6 h-6 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z" />
                        </svg>
                    </div>
                    <div>
                        <h3 class="text-lg font-semibold text-white">{$_('security.twoFactor')}</h3>
                        <p class="text-dark-400 text-sm mt-1">
                            {$_('security.twoFactorDescription')}
                        </p>
                        {#if twoFactorStatus?.enabled}
                            <div class="flex items-center gap-2 mt-3">
                                <span class="inline-flex items-center px-2.5 py-1 text-xs font-medium rounded-full bg-emerald-500/20 text-emerald-400">
                                    <svg class="w-3.5 h-3.5 mr-1" fill="currentColor" viewBox="0 0 20 20">
                                        <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
                                    </svg>
                                    {$_('security.enabled')}
                                </span>
                                {#if twoFactorStatus.verifiedAt}
                                    <span class="text-xs text-dark-500">
                                        Enabled {new Date(twoFactorStatus.verifiedAt).toLocaleDateString()}
                                    </span>
                                {/if}
                            </div>
                        {:else}
                            <span class="inline-flex items-center px-2.5 py-1 mt-3 text-xs font-medium rounded-full bg-red-500/20 text-red-400">
                                <svg class="w-3.5 h-3.5 mr-1" fill="currentColor" viewBox="0 0 20 20">
                                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
                                </svg>
                                {$_('security.disabled')}
                            </span>
                        {/if}
                    </div>
                </div>
                <div class="flex gap-2">
                    {#if twoFactorStatus?.enabled}
                        <button
                            on:click={() => showRegenerateModal = true}
                            class="btn-secondary text-sm"
                        >
                            {$_('security.regenerateBackupCodes')}
                        </button>
                        <button
                            on:click={() => showDisableModal = true}
                            class="btn-danger text-sm"
                        >
                            {$_('security.disable')}
                        </button>
                    {:else}
                        <button
                            on:click={() => showSetupModal = true}
                            class="btn-primary"
                        >
                            {$_('security.enable2FA')}
                        </button>
                    {/if}
                </div>
            </div>
        </div>

        <!-- Security Tips -->
        <div class="card p-6 border-primary-500/20">
            <h3 class="text-lg font-semibold text-white mb-4">{$_('security.tips')}</h3>
            <ul class="space-y-3 text-sm text-dark-400">
                <li class="flex items-start gap-3">
                    <svg class="w-5 h-5 text-primary-400 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <span>{$_('security.tip1')}</span>
                </li>
                <li class="flex items-start gap-3">
                    <svg class="w-5 h-5 text-primary-400 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <span>{$_('security.tip2')}</span>
                </li>
                <li class="flex items-start gap-3">
                    <svg class="w-5 h-5 text-primary-400 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <span>{$_('security.tip3')}</span>
                </li>
            </ul>
        </div>
    {/if}
</div>

<!-- Setup 2FA Modal -->
{#if showSetupModal}
    <div class="fixed inset-0 z-50 overflow-y-auto">
        <div class="fixed inset-0 bg-dark-950/80 backdrop-blur-sm" on:click={closeSetupModal} on:keydown={(e) => e.key === 'Escape' && closeSetupModal()} role="button" tabindex="-1"></div>
        <div class="flex min-h-full items-center justify-center p-4">
            <div class="relative w-full max-w-md card p-6 animate-slide-up">
                <button on:click={closeSetupModal} class="absolute top-4 right-4 p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-700/50">
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>

                {#if setupStep === 'password'}
                    <div class="text-center mb-6">
                        <div class="w-16 h-16 rounded-2xl bg-primary-500/10 flex items-center justify-center mx-auto mb-4">
                            <svg class="w-8 h-8 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M16.5 10.5V6.75a4.5 4.5 0 10-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 002.25-2.25v-6.75a2.25 2.25 0 00-2.25-2.25H6.75a2.25 2.25 0 00-2.25 2.25v6.75a2.25 2.25 0 002.25 2.25z" />
                            </svg>
                        </div>
                        <h3 class="text-xl font-semibold text-white">{$_('security.enable2FA')}</h3>
                        <p class="text-dark-400 text-sm mt-2">Enter your password to continue</p>
                    </div>

                    <form on:submit|preventDefault={startSetup} class="space-y-4">
                        <div class="input-group">
                            <label for="setup-password" class="input-label">{$_('auth.password')}</label>
                            <input
                                type="password"
                                id="setup-password"
                                bind:value={setupPassword}
                                class="input"
                                placeholder={$_('auth.enterPassword')}
                                required
                            />
                        </div>
                        <button type="submit" class="btn-primary w-full" disabled={setupLoading}>
                            {#if setupLoading}
                                <span class="spinner"></span>
                            {:else}
                                {$_('common.continue')}
                            {/if}
                        </button>
                    </form>

                {:else if setupStep === 'qrcode'}
                    <div class="text-center mb-4">
                        <h3 class="text-xl font-semibold text-white">{$_('security.scanQRCode')}</h3>
                        <p class="text-dark-400 text-sm mt-2">Scan this QR code with your authenticator app</p>
                    </div>

                    <div class="flex flex-col items-center space-y-4">
                        <div class="bg-white p-3 rounded-xl">
                            <img src={qrCode} alt="2FA QR Code" class="w-40 h-40" />
                        </div>

                        <details class="w-full">
                            <summary class="text-dark-400 text-xs cursor-pointer hover:text-dark-300">
                                Can't scan? Enter code manually
                            </summary>
                            <div class="bg-dark-800 p-3 rounded-lg mt-2">
                                <code class="text-primary-400 text-xs break-all font-mono">{secret}</code>
                            </div>
                        </details>

                        <p class="text-dark-500 text-xs">
                            Compatible with Google Authenticator, Authy, 1Password, etc.
                        </p>
                    </div>

                    <!-- Verification input right below QR code -->
                    <div class="mt-6 pt-6 border-t border-dark-700/50">
                        <form on:submit|preventDefault={verifySetup} class="space-y-4">
                            <div class="input-group">
                                <label for="verify-code" class="input-label">{$_('security.verificationCode')}</label>
                                <input
                                    type="text"
                                    id="verify-code"
                                    bind:value={setupCode}
                                    class="input text-center tracking-[0.5em] font-mono text-lg"
                                    placeholder="000000"
                                    maxlength="6"
                                    inputmode="numeric"
                                    pattern="[0-9]*"
                                    autocomplete="one-time-code"
                                    required
                                />
                                <p class="text-xs text-dark-500 mt-1">Enter the 6-digit code from your authenticator app</p>
                            </div>
                            <button type="submit" class="btn-primary w-full" disabled={setupLoading || setupCode.length < 6}>
                                {#if setupLoading}
                                    <span class="spinner"></span>
                                {:else}
                                    {$_('security.verify')}
                                {/if}
                            </button>
                        </form>
                    </div>

                {:else if setupStep === 'backup'}
                    <div class="text-center mb-6">
                        <div class="w-16 h-16 rounded-2xl bg-emerald-500/10 flex items-center justify-center mx-auto mb-4">
                            <svg class="w-8 h-8 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                            </svg>
                        </div>
                        <h3 class="text-xl font-semibold text-white">{$_('security.saveBackupCodes')}</h3>
                        <p class="text-dark-400 text-sm mt-2">{$_('security.backupCodesDescription')}</p>
                    </div>

                    <div class="bg-dark-800 p-4 rounded-lg mb-4">
                        <div class="grid grid-cols-2 gap-2">
                            {#each backupCodes as code}
                                <code class="text-primary-400 text-sm font-mono bg-dark-900 px-3 py-2 rounded text-center">{code}</code>
                            {/each}
                        </div>
                    </div>

                    <div class="flex gap-2 mb-4">
                        <button on:click={copyBackupCodes} class="btn-secondary flex-1">
                            <svg class="w-4 h-4 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M15.666 3.888A2.25 2.25 0 0013.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 01-.75.75H9a.75.75 0 01-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 01-2.25 2.25H6.75A2.25 2.25 0 014.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 011.927-.184" />
                            </svg>
                            {$_('common.copy')}
                        </button>
                        <button on:click={downloadBackupCodes} class="btn-secondary flex-1">
                            <svg class="w-4 h-4 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5M16.5 12L12 16.5m0 0L7.5 12m4.5 4.5V3" />
                            </svg>
                            {$_('common.download')}
                        </button>
                    </div>

                    <div class="flex items-start gap-2 p-3 rounded-lg bg-amber-500/10 border border-amber-500/20 mb-4">
                        <svg class="w-5 h-5 text-amber-400 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
                        </svg>
                        <p class="text-amber-400 text-xs">{$_('security.backupCodesWarning')}</p>
                    </div>

                    <button on:click={closeSetupModal} class="btn-primary w-full">
                        {$_('common.done')}
                    </button>
                {/if}
            </div>
        </div>
    </div>
{/if}

<!-- Disable 2FA Modal -->
{#if showDisableModal}
    <div class="fixed inset-0 z-50 overflow-y-auto">
        <div class="fixed inset-0 bg-dark-950/80 backdrop-blur-sm" on:click={() => showDisableModal = false} on:keydown={(e) => e.key === 'Escape' && (showDisableModal = false)} role="button" tabindex="-1"></div>
        <div class="flex min-h-full items-center justify-center p-4">
            <div class="relative w-full max-w-md card p-6 animate-slide-up">
                <div class="text-center mb-6">
                    <div class="w-16 h-16 rounded-2xl bg-red-500/10 flex items-center justify-center mx-auto mb-4">
                        <svg class="w-8 h-8 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
                        </svg>
                    </div>
                    <h3 class="text-xl font-semibold text-white">{$_('security.disable2FA')}</h3>
                    <p class="text-dark-400 text-sm mt-2">{$_('security.disable2FAWarning')}</p>
                </div>

                <form on:submit|preventDefault={disable2FA} class="space-y-4">
                    <div class="input-group">
                        <label for="disable-password" class="input-label">{$_('auth.password')}</label>
                        <input
                            type="password"
                            id="disable-password"
                            bind:value={disablePassword}
                            class="input"
                            placeholder={$_('auth.enterPassword')}
                            required
                        />
                    </div>
                    <div class="input-group">
                        <label for="disable-code" class="input-label">{$_('security.verificationCode')}</label>
                        <input
                            type="text"
                            id="disable-code"
                            bind:value={disableCode}
                            class="input text-center tracking-[0.5em] font-mono text-lg"
                            placeholder="000000"
                            maxlength="6"
                            required
                        />
                    </div>
                    <div class="flex gap-2">
                        <button type="button" on:click={() => showDisableModal = false} class="btn-secondary flex-1">
                            {$_('common.cancel')}
                        </button>
                        <button type="submit" class="btn-danger flex-1" disabled={disableLoading}>
                            {#if disableLoading}
                                <span class="spinner"></span>
                            {:else}
                                {$_('security.disable')}
                            {/if}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    </div>
{/if}

<!-- Regenerate Backup Codes Modal -->
{#if showRegenerateModal}
    <div class="fixed inset-0 z-50 overflow-y-auto">
        <div class="fixed inset-0 bg-dark-950/80 backdrop-blur-sm" on:click={() => { showRegenerateModal = false; newBackupCodes = []; }} on:keydown={(e) => e.key === 'Escape' && (showRegenerateModal = false)} role="button" tabindex="-1"></div>
        <div class="flex min-h-full items-center justify-center p-4">
            <div class="relative w-full max-w-md card p-6 animate-slide-up">
                {#if newBackupCodes.length === 0}
                    <div class="text-center mb-6">
                        <h3 class="text-xl font-semibold text-white">{$_('security.regenerateBackupCodes')}</h3>
                        <p class="text-dark-400 text-sm mt-2">{$_('security.regenerateWarning')}</p>
                    </div>

                    <form on:submit|preventDefault={regenerateBackupCodes} class="space-y-4">
                        <div class="input-group">
                            <label for="regenerate-code" class="input-label">{$_('security.verificationCode')}</label>
                            <input
                                type="text"
                                id="regenerate-code"
                                bind:value={regenerateCode}
                                class="input text-center tracking-[0.5em] font-mono text-lg"
                                placeholder="000000"
                                maxlength="6"
                                required
                            />
                        </div>
                        <div class="flex gap-2">
                            <button type="button" on:click={() => showRegenerateModal = false} class="btn-secondary flex-1">
                                {$_('common.cancel')}
                            </button>
                            <button type="submit" class="btn-primary flex-1" disabled={regenerateLoading}>
                                {#if regenerateLoading}
                                    <span class="spinner"></span>
                                {:else}
                                    {$_('security.regenerate')}
                                {/if}
                            </button>
                        </div>
                    </form>
                {:else}
                    <div class="text-center mb-6">
                        <h3 class="text-xl font-semibold text-white">{$_('security.newBackupCodes')}</h3>
                        <p class="text-dark-400 text-sm mt-2">{$_('security.backupCodesDescription')}</p>
                    </div>

                    <div class="bg-dark-800 p-4 rounded-lg mb-4">
                        <div class="grid grid-cols-2 gap-2">
                            {#each newBackupCodes as code}
                                <code class="text-primary-400 text-sm font-mono bg-dark-900 px-3 py-2 rounded text-center">{code}</code>
                            {/each}
                        </div>
                    </div>

                    <div class="flex gap-2 mb-4">
                        <button on:click={copyBackupCodes} class="btn-secondary flex-1">
                            {$_('common.copy')}
                        </button>
                        <button on:click={downloadBackupCodes} class="btn-secondary flex-1">
                            {$_('common.download')}
                        </button>
                    </div>

                    <button on:click={() => { showRegenerateModal = false; newBackupCodes = []; }} class="btn-primary w-full">
                        {$_('common.done')}
                    </button>
                {/if}
            </div>
        </div>
    </div>
{/if}

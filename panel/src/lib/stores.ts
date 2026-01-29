import { writable, derived, get } from 'svelte/store';
import type { User } from './types';

export const user = writable<User | null>(null);
export const token = writable<string | null>(null);

if (typeof window !== 'undefined') {
    const savedToken = localStorage.getItem('token');
    const savedUser = localStorage.getItem('user');
    if (savedToken) token.set(savedToken);
    if (savedUser) user.set(JSON.parse(savedUser));
}

token.subscribe((value) => {
    if (typeof window !== 'undefined') {
        if (value) localStorage.setItem('token', value);
        else localStorage.removeItem('token');
    }
});

user.subscribe((value) => {
    if (typeof window !== 'undefined') {
        if (value) localStorage.setItem('user', JSON.stringify(value));
        else localStorage.removeItem('user');
    }
});

// Permission helpers
export const isAdmin = derived(user, ($user) => {
    if (!$user) return false;
    return $user.roleName === 'admin' || $user.permissions?.['*'] === true;
});

export const isManager = derived(user, ($user) => {
    if (!$user) return false;
    return $user.roleName === 'manager' || $user.roleName === 'admin' || $user.permissions?.['*'] === true;
});

// Check if user can create containers
export const canCreateContainers = derived(user, ($user) => {
    if (!$user) return false;
    if ($user.permissions?.['*'] === true) return true;
    if ($user.roleName === 'admin' || $user.roleName === 'manager') return true;
    return $user.permissions?.['containers.create'] === true;
});

// Check if user can view daemons page
export const canViewDaemons = derived(user, ($user) => {
    if (!$user) return false;
    if ($user.permissions?.['*'] === true) return true;
    if ($user.roleName === 'admin' || $user.roleName === 'manager') return true;
    return $user.permissions?.['daemons.view'] === true;
});

export function hasPermission(permission: string): boolean {
    const $user = get(user);
    if (!$user) return false;
    if ($user.permissions?.['*'] === true) return true;
    return $user.permissions?.[permission] === true;
}

export function canAccessAdmin(): boolean {
    const $user = get(user);
    if (!$user) return false;
    return $user.roleName === 'admin' || $user.roleName === 'manager' || $user.permissions?.['admin.access'] === true;
}


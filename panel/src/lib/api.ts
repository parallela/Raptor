import { get } from 'svelte/store';
import { token } from './stores';
import type { Container, Daemon, Allocation, ContainerAllocation, User, ResourceLimits, ContainerPort } from './types';

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000';

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
    const t = get(token);
    const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        ...(options.headers as Record<string, string>),
    };
    
    if (t) {
        headers['Authorization'] = `Bearer ${t}`;
    }

    const res = await fetch(`${API_URL}${path}`, {
        ...options,
        headers,
    });

    if (!res.ok) {
        const error = await res.json().catch(() => ({ error: res.statusText }));
        throw new Error(error.error || res.statusText);
    }

    return res.json();
}

async function requestText(path: string): Promise<string> {
    const t = get(token);
    const headers: Record<string, string> = {};

    if (t) {
        headers['Authorization'] = `Bearer ${t}`;
    }

    const res = await fetch(`${API_URL}${path}`, { headers });

    if (!res.ok) {
        const error = await res.json().catch(() => ({ error: res.statusText }));
        throw new Error(error.error || res.statusText);
    }

    return res.text();
}

export interface CreateContainerData {
    daemonId: string;
    name: string;
    image: string;
    startupScript?: string;
    allocationId?: string;
    // Resource limits
    memoryLimit?: number;
    cpuLimit?: number;
    diskLimit?: number;
    swapLimit?: number;
    ioWeight?: number;
    // Optional user assignment (admin only)
    userId?: string;
}

export const api = {
    // Auth
    login: (username: string, password: string) =>
        request<{ token: string; user: User }>('/auth/login', {
            method: 'POST',
            body: JSON.stringify({ username, password }),
        }),
    
    register: (username: string, email: string, password: string) =>
        request<User>('/auth/register', {
            method: 'POST',
            body: JSON.stringify({ username, email, password }),
        }),

    forgotPassword: (email: string) =>
        request<{ message: string }>('/auth/forgot-password', {
            method: 'POST',
            body: JSON.stringify({ email }),
        }),

    resetPassword: (token: string, password: string) =>
        request<{ message: string }>('/auth/reset-password', {
            method: 'POST',
            body: JSON.stringify({ token, password }),
        }),

    // Containers
    listContainers: () => request<Container[]>('/containers'),
    getContainer: (id: string) => request<Container>(`/containers/${id}`),
    getContainerPorts: (id: string) => request<ContainerPort[]>(`/containers/${id}/ports`),
    createContainer: (data: CreateContainerData) =>
        request<Container>('/containers', { method: 'POST', body: JSON.stringify(data) }),
    updateContainer: (id: string, data: Partial<CreateContainerData>) =>
        request<Container>(`/containers/${id}`, { method: 'PATCH', body: JSON.stringify(data) }),
    deleteContainer: (id: string) => request<void>(`/containers/${id}`, { method: 'DELETE' }),
    assignAllocation: (id: string, allocationId: string) =>
        request<{ message: string; allocationIp: string; allocationPort: number }>(`/containers/${id}/allocation`, {
            method: 'POST',
            body: JSON.stringify({ allocationId })
        }),
    getContainerAllocations: (id: string) => request<ContainerAllocation[]>(`/containers/${id}/allocations`),
    getAvailableAllocations: (id: string) => request<Allocation[]>(`/containers/${id}/allocations/available`),
    addAllocation: (id: string, allocationId: string) =>
        request<{ message: string; allocationIp: string; allocationPort: number }>(`/containers/${id}/allocations`, {
            method: 'POST',
            body: JSON.stringify({ allocationId })
        }),
    removeAllocation: (id: string, allocationId: string) =>
        request<{ message: string }>(`/containers/${id}/allocations/${allocationId}`, { method: 'DELETE' }),
    startContainer: (id: string) => request<Container>(`/containers/${id}/start`, { method: 'POST' }),
    stopContainer: (id: string) => request<Container>(`/containers/${id}/stop`, { method: 'POST' }),
    restartContainer: (id: string) => request<Container>(`/containers/${id}/restart`, { method: 'POST' }),
    killContainer: (id: string) => request<Container>(`/containers/${id}/kill`, { method: 'POST' }),
    sendCommand: (id: string, command: string) =>
        request<{ success: boolean }>(`/containers/${id}/command`, {
            method: 'POST',
            body: JSON.stringify({ command })
        }),
    gracefulStop: (id: string, timeoutSecs: number = 30) =>
        request<{ success: boolean }>(`/containers/${id}/graceful-stop`, {
            method: 'POST',
            body: JSON.stringify({ timeoutSecs })
        }),
    getContainerStats: (id: string) => request<{ cpuPercent: number; memoryUsage: number; memoryLimit: number; memoryPercent: number; networkRx: number; networkTx: number; blockRead: number; blockWrite: number }>(`/containers/${id}/stats`),
    setSftpPassword: (id: string, password: string) =>
        request<{ message: string }>(`/containers/${id}/sftp-password`, {
            method: 'POST',
            body: JSON.stringify({ password })
        }),

    // Daemons
    listDaemons: () => request<Daemon[]>('/daemons'),
    getDaemon: (id: string) => request<Daemon>(`/daemons/${id}`),
    getDaemonStatus: (id: string) => request<{ id: string; status: string; system?: { totalMemory: number; availableMemory: number; cpuCores: number; cpuUsage: number; totalDisk: number; availableDisk: number; hostname: string } }>(`/daemons/${id}/status`),
    createDaemon: (data: { name: string; host: string; port: number; location?: string; totalMemory?: number; totalCpu?: number; totalDisk?: number }) =>
        request<Daemon>('/daemons', { method: 'POST', body: JSON.stringify(data) }),
    updateDaemon: (id: string, data: Partial<Daemon>) =>
        request<Daemon>(`/daemons/${id}`, { method: 'PATCH', body: JSON.stringify(data) }),
    deleteDaemon: (id: string) => request<void>(`/daemons/${id}`, { method: 'DELETE' }),

    // Allocations
    listAllocations: () => request<Allocation[]>('/allocations'),
    createAllocation: (data: { daemonId: string; ip: string; port: number }) =>
        request<Allocation>('/allocations', { method: 'POST', body: JSON.stringify(data) }),
    deleteAllocation: (id: string) => request<void>(`/allocations/${id}`, { method: 'DELETE' }),

    // Users
    getMe: () => request<User>('/users/me'),
    listUsers: () => request<User[]>('/users'),
    getUser: (id: string) => request<User>(`/users/${id}`),
    updateUser: (id: string, data: { username?: string; roleId?: string }) =>
        request<User>(`/users/${id}`, { method: 'PATCH', body: JSON.stringify(data) }),
    deleteUser: (id: string) => request<void>(`/users/${id}`, { method: 'DELETE' }),
    searchUsers: (query: string, limit: number = 10) =>
        request<User[]>(`/admin/users/search?q=${encodeURIComponent(query)}&limit=${limit}`),
    inviteUser: (email: string, roleId?: string) =>
        request<{ message: string; token: string }>('/admin/users/invite', {
            method: 'POST',
            body: JSON.stringify({ email, roleId })
        }),
    acceptInvite: (token: string, username: string, password: string) =>
        request<User>('/auth/accept-invite', {
            method: 'POST',
            body: JSON.stringify({ token, username, password })
        }),

    // Roles
    listRoles: () => request<{ id: string; name: string; permissions: Record<string, boolean> }[]>('/roles'),
    getRole: (id: string) => request<{ id: string; name: string; permissions: Record<string, boolean> }>(`/roles/${id}`),
    createRole: (data: { name: string; permissions: Record<string, boolean> }) =>
        request<{ id: string; name: string; permissions: Record<string, boolean> }>('/roles', { method: 'POST', body: JSON.stringify(data) }),
    updateRole: (id: string, data: { name: string; permissions: Record<string, boolean> }) =>
        request<{ id: string; name: string; permissions: Record<string, boolean> }>(`/roles/${id}`, { method: 'PATCH', body: JSON.stringify(data) }),
    deleteRole: (id: string) => request<void>(`/roles/${id}`, { method: 'DELETE' }),

    // Files
    listFiles: (containerId: string, path: string = '/') =>
        request<{ name: string; isDir: boolean; size: number; modified?: string }[]>(`/containers/${containerId}/files?path=${encodeURIComponent(path)}`),
    readFile: (containerId: string, path: string) =>
        requestText(`/containers/${containerId}/files/read?path=${encodeURIComponent(path)}`),
    writeFile: (containerId: string, path: string, content: string) =>
        request<{ message: string }>(`/containers/${containerId}/files/write`, { method: 'POST', body: JSON.stringify({ path, content }) }),
    createFolder: (containerId: string, path: string) =>
        request<{ message: string }>(`/containers/${containerId}/files/folder`, { method: 'POST', body: JSON.stringify({ path }) }),
    deleteFile: (containerId: string, path: string) =>
        request<{ message: string }>(`/containers/${containerId}/files/delete?path=${encodeURIComponent(path)}`, { method: 'DELETE' }),

    // Admin: All containers
    listAllContainers: () => request<Container[]>('/admin/containers'),
};

export function createWebSocket(containerId: string): WebSocket {
    const t = get(token);
    const wsUrl = (import.meta.env.VITE_API_URL || 'http://localhost:3000')
        .replace('http', 'ws');
    return new WebSocket(`${wsUrl}/ws/containers/${containerId}/logs?token=${t}`);
}

export function createStatsWebSocket(containerId: string): WebSocket {
    const t = get(token);
    const wsUrl = (import.meta.env.VITE_API_URL || 'http://localhost:3000')
        .replace('http', 'ws');
    return new WebSocket(`${wsUrl}/ws/containers/${containerId}/stats?token=${t}`);
}

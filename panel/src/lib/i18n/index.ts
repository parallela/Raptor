import { writable, derived, get } from 'svelte/store';
import en from './locales/en.json';

// Available locales
export const locales = ['en', 'bg', 'de', 'es', 'fr', 'ru', 'zh'] as const;
export type Locale = (typeof locales)[number];

// Locale display names
export const localeNames: Record<Locale, string> = {
    en: 'English',
    bg: 'Български',
    de: 'Deutsch',
    es: 'Español',
    fr: 'Français',
    ru: 'Русский',
    zh: '中文'
};

// Type for translation keys
type TranslationKeys = typeof en;
type NestedKeyOf<T> = T extends object
    ? { [K in keyof T]: K extends string ? (T[K] extends object ? `${K}.${NestedKeyOf<T[K]>}` : K) : never }[keyof T]
    : never;
export type TranslationKey = NestedKeyOf<TranslationKeys>;

// Current locale store
const storedLocale = typeof localStorage !== 'undefined' ? localStorage.getItem('locale') : null;
export const locale = writable<Locale>((storedLocale as Locale) || 'en');

// Subscribe to locale changes and persist
locale.subscribe((value) => {
    if (typeof localStorage !== 'undefined') {
        localStorage.setItem('locale', value);
    }
});

// Translations cache
const translations: Record<string, typeof en> = { en };

// Load translations for a locale
async function loadTranslations(loc: Locale): Promise<typeof en> {
    if (translations[loc]) {
        return translations[loc];
    }

    try {
        const module = await import(`./locales/${loc}.json`);
        translations[loc] = module.default;
        return module.default;
    } catch (e) {
        console.warn(`Failed to load translations for locale: ${loc}, falling back to English`);
        return en;
    }
}

// Current translations store
export const currentTranslations = writable<typeof en>(en);

// Update translations when locale changes
locale.subscribe(async (loc) => {
    const trans = await loadTranslations(loc);
    currentTranslations.set(trans);
});

// Get nested value from object by dot notation path
function getNestedValue(obj: Record<string, unknown>, path: string): string | undefined {
    const keys = path.split('.');
    let current: unknown = obj;

    for (const key of keys) {
        if (current && typeof current === 'object' && key in current) {
            current = (current as Record<string, unknown>)[key];
        } else {
            return undefined;
        }
    }

    return typeof current === 'string' ? current : undefined;
}

// Translation function
export function t(key: string, params?: Record<string, string | number>): string {
    const trans = get(currentTranslations);
    let value = getNestedValue(trans as unknown as Record<string, unknown>, key);

    if (!value) {
        // Fallback to English
        value = getNestedValue(en as unknown as Record<string, unknown>, key);
    }

    if (!value) {
        console.warn(`Missing translation for key: ${key}`);
        return key;
    }

    // Replace parameters
    if (params) {
        for (const [paramKey, paramValue] of Object.entries(params)) {
            value = value.replace(new RegExp(`\\{${paramKey}\\}`, 'g'), String(paramValue));
        }
    }

    return value;
}

// Reactive translation function for use in components
export const _ = derived(currentTranslations, ($trans) => {
    return (key: string, params?: Record<string, string | number>): string => {
        let value = getNestedValue($trans as unknown as Record<string, unknown>, key);

        if (!value) {
            value = getNestedValue(en as unknown as Record<string, unknown>, key);
        }

        if (!value) {
            return key;
        }

        if (params) {
            for (const [paramKey, paramValue] of Object.entries(params)) {
                value = value.replace(new RegExp(`\\{${paramKey}\\}`, 'g'), String(paramValue));
            }
        }

        return value;
    };
});

// Set locale
export function setLocale(loc: Locale): void {
    if (locales.includes(loc)) {
        locale.set(loc);
    }
}

// Get current locale
export function getLocale(): Locale {
    return get(locale);
}

// Initialize - call this on app mount
export async function initI18n(): Promise<void> {
    const currentLocale = get(locale);
    await loadTranslations(currentLocale);
}

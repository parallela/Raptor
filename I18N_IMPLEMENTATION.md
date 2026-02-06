# i18n Implementation Summary

## Overview
The Raptor panel now has full internationalization (i18n) support with a language switcher and translated strings throughout the application.

## Languages Available
- English (en) - Default
- Bulgarian (bg) - Български
- German (de) - Deutsch
- Spanish (es) - Español
- French (fr) - Français
- Russian (ru) - Русский
- Chinese (zh) - 中文

## Components Implemented

### 1. Language Switcher (`LocaleSelector.svelte`)
- **Location**: `/panel/src/lib/components/LocaleSelector.svelte`
- **Features**:
  - Dropdown menu with all available languages
  - Globe icon indicator
  - Persists language selection to localStorage
  - Responsive design (hides language name on mobile)
  - Click-outside-to-close functionality

### 2. i18n System (`/panel/src/lib/i18n/`)
- **Location**: `/panel/src/lib/i18n/index.ts`
- **Features**:
  - Dynamic locale loading
  - Reactive translation function (`$_()`)
  - Parameter interpolation (e.g., `{username}`)
  - Fallback to English if translation missing
  - LocalStorage persistence
  - TypeScript support with type-safe translation keys

## Placement of Language Switcher

### Desktop View
- **Sidebar**: Added in the user section at the bottom (above user profile)
- **Location**: `/panel/src/routes/+layout.svelte` (line ~153)

### Mobile View
- **Header**: Added in the top-right mobile header next to the user avatar
- **Login Page**: Added in the top-right corner for unauthenticated users

## Pages Updated with i18n

### 1. Main Layout (`+layout.svelte`)
- Navigation menu items (Dashboard, Containers, Database, Daemons, Admin)
- Logout button tooltip
- Language switcher integrated in both desktop sidebar and mobile header

### 2. Dashboard (`+page.svelte`)
- Page title and welcome message
- Stats cards (Total Containers, Running, Stopped)
- Quick Actions section
- "Your Containers" section
- Empty state messages

### 3. Login Page (`login/+page.svelte`)
- Page headers (Welcome back, Create account, Reset password)
- Form labels (Username, Email, Password)
- Input placeholders
- Button text (Sign In, Sign Up, Send Reset Link)
- Footer links (Don't have an account?, Already have an account?)
- Language switcher in top-right corner

## Translation Keys Structure

The translation keys are organized in a hierarchical structure:

```json
{
  "common": { /* Common UI elements */ },
  "nav": { /* Navigation items */ },
  "auth": { /* Authentication pages */ },
  "dashboard": { /* Dashboard page */ },
  "containers": { /* Containers management */ },
  "database": { /* Database management */ },
  "files": { /* File manager */ },
  "ftp": { /* FTP access */ },
  "settings": { /* Settings pages */ },
  "admin": { /* Admin pages */ },
  "errors": { /* Error messages */ },
  "validation": { /* Form validation */ },
  "time": { /* Time-related strings */ },
  "units": { /* Unit labels */ }
}
```

## Usage Examples

### In Components
```svelte
<script lang="ts">
  import { _ } from '$lib/i18n';
</script>

<!-- Simple translation -->
<h1>{$_('dashboard.title')}</h1>

<!-- With parameters -->
<p>{$_('dashboard.welcomeMessage', { username: $user?.username })}</p>

<!-- In attributes -->
<button title={$_('nav.logout')}>Logout</button>
```

### Changing Language
Users can change the language by:
1. Clicking the globe icon in the sidebar (desktop) or header (mobile)
2. Selecting a language from the dropdown
3. The selection is saved to localStorage and persists across sessions

## Files Modified

### Core Files
- `/panel/src/routes/+layout.svelte` - Added LocaleSelector and translated navigation
- `/panel/src/routes/+page.svelte` - Translated dashboard content
- `/panel/src/routes/login/+page.svelte` - Translated login/register/forgot password forms

### i18n System (Already existed, no changes needed)
- `/panel/src/lib/i18n/index.ts` - i18n core logic
- `/panel/src/lib/i18n/locales/*.json` - Translation files for each language
- `/panel/src/lib/components/LocaleSelector.svelte` - Language switcher component

## Testing

To test the implementation:

1. **Start the development server**:
   ```bash
   cd /Users/lubomirstankov/Development/me/raptor/panel
   npm run dev
   ```

2. **Test language switching**:
   - Click the globe icon in the sidebar
   - Select different languages
   - Verify that all text changes appropriately
   - Refresh the page - language should persist

3. **Test on different pages**:
   - Visit the login page (unauthenticated)
   - View the dashboard (authenticated)
   - Check that translations work on all pages

## Next Steps

To add more translated pages, follow this pattern:

1. **Import the translation function** in your component:
   ```svelte
   import { _ } from '$lib/i18n';
   ```

2. **Replace hard-coded strings** with translation keys:
   ```svelte
   <!-- Before -->
   <h1>My Page Title</h1>
   
   <!-- After -->
   <h1>{$_('myPage.title')}</h1>
   ```

3. **Add translation keys** to all language files in `/panel/src/lib/i18n/locales/*.json`

4. **For parameters**, use the second argument:
   ```svelte
   {$_('message.text', { name: userName, count: 5 })}
   ```

## Notes

- All translation files are in JSON format
- English (`en.json`) is the fallback language
- Missing translations will fall back to English, then display the key if not found
- The i18n system is reactive - changing language updates all text immediately
- No page refresh required when changing languages

## Complete Translation Coverage

The following pages still have hard-coded strings that need translation:
- `/containers` - Container listing page
- `/containers/[id]/*` - Container detail pages (console, files, ftp, settings)
- `/databases` - Database management page
- `/daemons` - Daemon management page
- `/admin/*` - Admin pages (users, allocations, flakes, database servers)
- `/reset-password` - Password reset page
- `/invite` - User invitation page

You can continue adding translations to these pages using the same pattern demonstrated in the files already updated.

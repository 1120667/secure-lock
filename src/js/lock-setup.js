// SecureLock — Lock Setup
// Handles application selection and PIN configuration.

(function () {
    const { invoke } = window.__TAURI__.core;

    let apps = [];
    let selectedApps = [];

    /**
     * Fetch and display running applications.
     */
    async function loadApps() {
        const listEl = document.getElementById('app-list');
        listEl.innerHTML = `
            <div class="app-list-loading">
                <div class="spinner"></div>
                <span>Scanning running applications...</span>
            </div>
        `;

        try {
            apps = await invoke('get_running_apps');

            if (apps.length === 0) {
                listEl.innerHTML = `
                    <div class="app-list-loading">
                        <span>No visible applications found. Open an app and click Refresh.</span>
                    </div>
                `;
                return;
            }

            listEl.innerHTML = '';
            apps.forEach((app, index) => {
                const item = document.createElement('div');
                item.className = 'app-item';
                item.dataset.index = index;
                item.innerHTML = `
                    <div class="app-item-icon">
                        ${getAppEmoji(app.process_name)}
                    </div>
                    <div class="app-item-info">
                        <div class="app-item-title">${escapeHtml(app.title)}</div>
                        <div class="app-item-process">${escapeHtml(app.process_name)}</div>
                    </div>
                `;
                item.addEventListener('click', () => selectApp(index, item));
                listEl.appendChild(item);
            });
        } catch (e) {
            console.error('[SecureLock] Error loading apps:', e);
            listEl.innerHTML = `
                <div class="app-list-loading">
                    <span style="color: var(--error)">Error loading applications: ${escapeHtml(String(e))}</span>
                </div>
            `;
        }
    }

    /**
     * View toggle when selecting an application.
     */
    function selectApp(index, element) {
        const app = apps[index];
        const selectedIndex = selectedApps.findIndex(a => a.hwnd === app.hwnd);
        
        if (selectedIndex >= 0) {
            // Deselect
            selectedApps.splice(selectedIndex, 1);
            element.classList.remove('selected');
        } else {
            // Select
            selectedApps.push(app);
            element.classList.add('selected');
        }
        
        AppState.selectedApps = selectedApps;

        // Toggle Next button
        document.getElementById('btn-next-step').disabled = selectedApps.length === 0;
    }

    /**
     * Proceed to PIN setup step.
     */
    function goToPinSetup() {
        if (selectedApps.length === 0) return;
        
        showSetupStep('step-pin-setup');
        
        const previewText = selectedApps.length === 1 
            ? selectedApps[0].title 
            : `${selectedApps.length} Applications Selected`;
            
        document.querySelector('.selected-app-name').textContent = previewText;
        document.getElementById('pin-input').focus();
    }

    /**
     * Handle the Lock button click.
     */
    async function handleLock() {
        const pinInput = document.getElementById('pin-input');
        const pinConfirm = document.getElementById('pin-confirm');
        const pin = pinInput.value;
        const confirm = pinConfirm.value;

        // Validation
        if (pin.length < 4 || pin.length > 8) {
            showError('pin-error', 'PIN must be between 4 and 8 characters');
            return;
        }

        if (pin !== confirm) {
            showError('pin-error', 'PINs do not match');
            pinConfirm.focus();
            return;
        }

        if (selectedApps.length === 0) {
            showError('pin-error', 'Please select at least one application first');
            return;
        }

        hideError('pin-error');

        // Disable button
        const lockBtn = document.getElementById('btn-lock');
        lockBtn.disabled = true;
        lockBtn.innerHTML = '<div class="spinner"></div> Locking...';

        try {
            const result = await invoke('start_lock', {
                apps: AppState.selectedApps,
                pin: pinInput.value,
            });
            
            // Switch view immediately after Rust morphs the window
            if (window.switchToLockedView) window.switchToLockedView();

            console.log('[SecureLock] Lock started:', result);

            // Clear form
            pinInput.value = '';
            pinConfirm.value = '';

            // Update locked view in CSS/HTML
            const lockText = selectedApps.length === 1 ? selectedApps[0].title : `${selectedApps.length} Applications`;
            const appNameEl = document.getElementById('locked-app-name');
            if (appNameEl) appNameEl.textContent = lockText;
            AppState.isLocked = true;

            // Switch to locked view
            switchView('locked');
            startDurationTimer();
        } catch (e) {
            console.error('[SecureLock] Lock failed:', e);
            showError('pin-error', `Lock failed: ${e}`);
        } finally {
            lockBtn.disabled = false;
            lockBtn.innerHTML = `
                <svg width="18" height="18" viewBox="0 0 18 18" fill="currentColor">
                    <path d="M14 8H13V6C13 3.79 11.21 2 9 2C6.79 2 5 3.79 5 6V8H4C2.9 8 2 8.9 2 10V16C2 17.1 2.9 18 4 18H14C15.1 18 16 17.1 16 16V10C16 8.9 15.1 8 14 8ZM9 14C7.9 14 7 13.1 7 12C7 10.9 7.9 10 9 10C10.1 10 11 10.9 11 12C11 13.1 10.1 14 9 14ZM11 8H7V6C7 4.9 7.9 4 9 4C10.1 4 11 4.9 11 6V8Z"/>
                </svg>
                Lock System
            `;
        }
    }

    /**
     * Get an emoji icon for common process names.
     */
    function getAppEmoji(processName) {
        const name = processName.toLowerCase();
        if (name.includes('chrome') || name.includes('firefox') || name.includes('edge') || name.includes('brave'))
            return '🌐';
        if (name.includes('code') || name.includes('vscode')) return '💻';
        if (name.includes('notepad')) return '📝';
        if (name.includes('explorer')) return '📁';
        if (name.includes('word') || name.includes('winword')) return '📄';
        if (name.includes('excel')) return '📊';
        if (name.includes('powerpoint') || name.includes('powerpnt')) return '📑';
        if (name.includes('acrobat') || name.includes('pdf')) return '📕';
        if (name.includes('vlc') || name.includes('wmplayer') || name.includes('media')) return '🎬';
        if (name.includes('spotify') || name.includes('music')) return '🎵';
        if (name.includes('discord')) return '💬';
        if (name.includes('slack') || name.includes('teams')) return '💼';
        if (name.includes('terminal') || name.includes('cmd') || name.includes('powershell')) return '⬛';
        if (name.includes('paint')) return '🎨';
        if (name.includes('calc')) return '🔢';
        return '🪟';
    }

    /**
     * Escape HTML to prevent XSS.
     */
    function escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    // Event Listeners
    document.addEventListener('DOMContentLoaded', () => {
        // Load apps on startup
        loadApps();

        // Refresh button
        document.getElementById('btn-refresh-apps')?.addEventListener('click', loadApps);

        // Next Step button
        document.getElementById('btn-next-step')?.addEventListener('click', goToPinSetup);

        // Back button (PIN step → app select)
        document.getElementById('btn-back-to-apps')?.addEventListener('click', () => {
            showSetupStep('step-app-select');
        });

        // Change app button (in PIN step)
        document.getElementById('btn-change-app')?.addEventListener('click', () => {
            showSetupStep('step-app-select');
        });

        // Lock button
        document.getElementById('btn-lock')?.addEventListener('click', handleLock);

        // Enter key on PIN confirm
        document.getElementById('pin-confirm')?.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') handleLock();
        });
    });
})();

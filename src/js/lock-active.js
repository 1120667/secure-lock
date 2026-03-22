// SecureLock — Lock Active View
// Shows the locked status overlay and handles the unlock trigger.

(function () {
    const { invoke } = window.__TAURI__.core;

    // Navigate to unlock screen
    document.addEventListener('DOMContentLoaded', () => {
        document.getElementById('btn-show-unlock')?.addEventListener('click', async () => {
            // Expand the widget to fit the PIN pad (downwards only)
            await invoke('resize_overlay_widget', { width: 280, height: 520 });
            
            switchView('unlock');
            // Focus the hidden input for keyboard support
            setTimeout(() => {
                document.getElementById('unlock-pin-input')?.focus();
            }, 300);
        });
    });
})();

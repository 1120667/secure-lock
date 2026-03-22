// SecureLock — App Entry Point & View Router
// Manages view transitions and global state.

// Now handled via Rust morphing the window, no more separate overlay process.
const IS_OVERLAY = false;

const { invoke } = window.__TAURI__.core;

// Global app state
const AppState = {
    currentView: 'setup',
    selectedApp: null,
    isLocked: false,
    durationInterval: null,
};

/**
 * Switch between views with smooth transitions.
 * @param {'setup' | 'locked' | 'unlock'} viewName
 */
function switchView(viewName) {
    document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));

    const target = document.getElementById(`view-${viewName}`);
    if (target) {
        requestAnimationFrame(() => {
            target.classList.add('active');
        });
    }

    AppState.currentView = viewName;
    console.log(`[SecureLock] View switched to: ${viewName}`);
}

/**
 * Global API for Rust backend to trigger view morphing
 */
window.switchToLockedView = function() {
    switchView('locked');
    populateLockedView();
    document.body.classList.add('is-overlay');
    document.documentElement.classList.add('is-overlay');
};

window.switchToSetupView = function() {
    document.body.classList.remove('is-overlay');
    document.documentElement.classList.remove('is-overlay');
    switchView('setup');
    showSetupStep('step-app-select');
};

/**
 * Show a setup step.
 */
function showSetupStep(stepId) {
    document.querySelectorAll('.setup-step').forEach(s => s.classList.remove('active'));
    const step = document.getElementById(stepId);
    if (step) step.classList.add('active');
}

/**
 * Show/hide an error message.
 */
function showError(elementId, message) {
    const el = document.getElementById(elementId);
    if (el) {
        el.textContent = message;
        el.classList.remove('hidden');
    }
}

function hideError(elementId) {
    const el = document.getElementById(elementId);
    if (el) el.classList.add('hidden');
}

/**
 * Format seconds to MM:SS.
 */
function formatDuration(totalSeconds) {
    const mins = Math.floor(totalSeconds / 60).toString().padStart(2, '0');
    const secs = (totalSeconds % 60).toString().padStart(2, '0');
    return `${mins}:${secs}`;
}

/**
 * Show the success animation, then run callback.
 */
function showSuccessAnimation(callback) {
    const el = document.getElementById('unlock-success');
    el.classList.remove('hidden');
    setTimeout(() => {
        el.classList.add('hidden');
        if (callback) callback();
    }, 1200);
}

/**
 * Start the lock duration timer.
 */
function startDurationTimer(initialSecs = 0) {
    let seconds = initialSecs;
    const el = document.getElementById('locked-duration');
    if (!el) return;

    if (AppState.durationInterval) clearInterval(AppState.durationInterval);
    el.textContent = formatDuration(seconds);
    AppState.durationInterval = setInterval(() => {
        seconds++;
        el.textContent = formatDuration(seconds);
    }, 1000);
}

/**
 * Stop the duration timer.
 */
function stopDurationTimer() {
    if (AppState.durationInterval) {
        clearInterval(AppState.durationInterval);
        AppState.durationInterval = null;
    }
}

/**
 * Populate the locked view with current session info.
 */
async function populateLockedView() {
    try {
        const status = await invoke('get_lock_status');
        if (status.allowed_app_title) {
            const el = document.getElementById('locked-app-name');
            if (el) el.textContent = status.allowed_app_title;
        }
        startDurationTimer(status.locked_duration_secs || 0);
    } catch (e) {
        console.error('[SecureLock] Error fetching lock status:', e);
    }
}

// ─── Initialization ────────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', () => {
    console.log(`[SecureLock] App UI Init`);
    // Main setup window starts at view-setup by default
});

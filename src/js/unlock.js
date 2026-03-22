// SecureLock — Unlock Screen
// Handles PIN entry via numpad, dot visualization, verification, and success animation.

(function () {
    const { invoke } = window.__TAURI__.core;

    let currentPin = '';
    const MAX_PIN_LENGTH = 8;
    let cooldownInterval = null;

    /**
     * Update the visual PIN dots.
     */
    function updateDots() {
        const dots = document.querySelectorAll('#pin-dots .dot');
        dots.forEach((dot, i) => {
            dot.classList.toggle('filled', i < currentPin.length);
            dot.classList.remove('error');
        });
    }

    /**
     * Add a digit to the PIN.
     */
    function addDigit(digit) {
        if (currentPin.length >= MAX_PIN_LENGTH) return;
        currentPin += digit;
        updateDots();
        hideError('unlock-error');

        // Update hidden input
        document.getElementById('unlock-pin-input').value = currentPin;
    }

    /**
     * Clear the PIN.
     */
    function clearPin() {
        currentPin = '';
        updateDots();
        document.getElementById('unlock-pin-input').value = '';
        hideError('unlock-error');
    }

    /**
     * Show error animation on dots.
     */
    function showDotError() {
        const dots = document.querySelectorAll('#pin-dots .dot');
        dots.forEach(dot => {
            dot.classList.remove('filled');
            dot.classList.add('error');
        });

        setTimeout(() => {
            dots.forEach(dot => dot.classList.remove('error'));
            clearPin();
        }, 600);
    }

    /**
     * Submit the PIN for verification.
     */
    async function submitPin() {
        if (currentPin.length < 4) {
            showError('unlock-error', 'Enter at least 4 characters');
            return;
        }

        try {
            const result = await invoke('attempt_unlock', { pin: currentPin });

            if (result === true) {
                // Success — show animation. The Rust backend will:
                // 1. Close this overlay window
                // 2. Restore all windows
                // 3. Show the main SecureLock window
                stopDurationTimer();
                AppState.isLocked = false;

                showSuccessAnimation(() => {
                    clearPin();
                    if (window.switchToSetupView) window.switchToSetupView();
                });
            } else {
                // Wrong PIN
                showDotError();
                showError('unlock-error', 'Incorrect PIN');
                checkRateLimit();
            }
        } catch (e) {
            const errorStr = String(e);
            if (errorStr.includes('Too many failed attempts')) {
                showDotError();
                showError('unlock-error', 'Too many attempts');
                startCooldownDisplay(errorStr);
            } else {
                showDotError();
                showError('unlock-error', errorStr);
            }
        }
    }

    /**
     * Check rate limit status.
     */
    async function checkRateLimit() {
        try {
            const [attempts, cooldown] = await invoke('get_rate_limit_info');
            if (cooldown > 0) {
                startCooldownDisplay(`Try again in ${cooldown} seconds`);
            }
        } catch (e) {
            console.error('[SecureLock] Rate limit check error:', e);
        }
    }

    /**
     * Show cooldown timer.
     */
    function startCooldownDisplay(message) {
        const el = document.getElementById('unlock-cooldown');
        el.classList.remove('hidden');

        // Disable numpad
        document.querySelectorAll('.numpad-btn').forEach(btn => {
            btn.disabled = true;
            btn.style.opacity = '0.3';
        });

        let remaining = 30;
        // Try to parse remaining from message
        const match = message.match(/(\d+)\s*seconds/);
        if (match) remaining = parseInt(match[1]);

        el.textContent = `⏳ Locked out. Try again in ${remaining}s`;

        if (cooldownInterval) clearInterval(cooldownInterval);
        cooldownInterval = setInterval(() => {
            remaining--;
            if (remaining <= 0) {
                clearInterval(cooldownInterval);
                cooldownInterval = null;
                el.classList.add('hidden');
                hideError('unlock-error');

                // Re-enable numpad
                document.querySelectorAll('.numpad-btn').forEach(btn => {
                    btn.disabled = false;
                    btn.style.opacity = '1';
                });
            } else {
                el.textContent = `⏳ Locked out. Try again in ${remaining}s`;
            }
        }, 1000);
    }

    // Event Listeners
    document.addEventListener('DOMContentLoaded', () => {
        // Numpad button clicks
        document.querySelectorAll('.numpad-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const key = btn.dataset.key;
                if (key === 'clear') {
                    clearPin();
                } else if (key === 'submit') {
                    submitPin();
                } else {
                    addDigit(key);
                }
            });
        });

        // Keyboard input support
        document.addEventListener('keydown', (e) => {
            if (AppState.currentView !== 'unlock') return;

            if (e.key >= '0' && e.key <= '9') {
                addDigit(e.key);
            } else if (e.key === 'Backspace') {
                currentPin = currentPin.slice(0, -1);
                updateDots();
            } else if (e.key === 'Enter') {
                submitPin();
            } else if (e.key === 'Escape') {
                invoke('resize_overlay_widget', { width: 280, height: 72 });
                switchView('locked');
                clearPin();
            }
        });

        // Cancel button
        document.getElementById('btn-back-to-locked')?.addEventListener('click', async () => {
            await invoke('resize_overlay_widget', { width: 280, height: 72 });
            switchView('locked');
            clearPin();
        });
    });
})();

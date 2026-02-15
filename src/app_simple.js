console.log('app.js loading...');

// Wait for Tauri to be available
let retries = 0;
const maxRetries = 50;

function initApp() {
    retries++;
    
    if (window.__TAURI__) {
        console.log('✅ Tauri API found');
        startApp();
    } else if (retries < maxRetries) {
        console.log(`⏳ Waiting for Tauri API... (${retries}/${maxRetries})`);
        setTimeout(initApp, 100);
    } else {
        console.error('❌ Tauri API not available after timeout');
        document.body.innerHTML = '<div style="padding: 20px; color: red; font-family: system-ui;"><h2>Error</h2><p>Tauri API failed to load. Check the browser console.</p></div>';
    }
}

async function startApp() {
    console.log('Starting app...');
    
    try {
        const { invoke } = window.__TAURI__.core;
        
        // Test basic invoke
        console.log('Testing invoke...');
        const stats = await invoke('get_statistics');
        console.log('✅ Invoke works! Got statistics:', stats);
        
        // Load the actual UI
        await loadUI(invoke);
        console.log('✅ UI loaded successfully');
        
    } catch (error) {
        console.error('❌ Error:', error);
        document.body.innerHTML = `<div style="padding: 20px; color: red; font-family: system-ui;"><h2>Error</h2><p>${error.message}</p><pre>${error.stack}</pre></div>`;
    }
}

async function loadUI(invoke) {
    console.log('Loading UI...');
    
    // Get all cards
    const cards = await invoke('get_all_cards');
    console.log('Loaded cards:', cards);
    
    // Get statistics
    const stats = await invoke('get_statistics');
    console.log('Loaded stats:', stats);
    
    // Build UI
    const cardsHtml = cards.map(card => `
        <div style="padding: 12px; border: 1px solid #ddd; border-radius: 4px; margin-bottom: 8px;">
            <strong>${escapeHtml(card.prompt)}</strong><br>
            <em>${escapeHtml(card.answer)}</em>
        </div>
    `).join('');
    
    document.getElementById('app').innerHTML = `
        <div style="font-family: system-ui; padding: 20px; max-width: 1200px;">
            <h1>Teacha - Spaced Repetition</h1>
            
            <h2>Statistics</h2>
            <div style="display: grid; grid-template-columns: repeat(4, 1fr); gap: 16px; margin-bottom: 20px;">
                <div style="padding: 12px; background: #f0f0f0; border-radius: 4px;">
                    <div style="font-size: 12px; color: #666;">Total Cards</div>
                    <div style="font-size: 24px; font-weight: bold;">${stats.total_cards}</div>
                </div>
                <div style="padding: 12px; background: #f0f0f0; border-radius: 4px;">
                    <div style="font-size: 12px; color: #666;">Due Now</div>
                    <div style="font-size: 24px; font-weight: bold;">${stats.cards_due}</div>
                </div>
                <div style="padding: 12px; background: #f0f0f0; border-radius: 4px;">
                    <div style="font-size: 12px; color: #666;">Learning</div>
                    <div style="font-size: 24px; font-weight: bold;">${stats.cards_learning}</div>
                </div>
                <div style="padding: 12px; background: #f0f0f0; border-radius: 4px;">
                    <div style="font-size: 12px; color: #666;">Review</div>
                    <div style="font-size: 24px; font-weight: bold;">${stats.cards_review}</div>
                </div>
            </div>
            
            <h2>Cards (${cards.length})</h2>
            <div>${cardsHtml}</div>
            
            <button onclick="location.reload()" style="padding: 8px 16px; margin-top: 20px; border-radius: 4px; border: none; background: #007AFF; color: white; cursor: pointer;">
                Reload
            </button>
        </div>
    `;
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Start the app
console.log('Initializing...');
initApp();

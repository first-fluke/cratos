/**
 * Cratos WhatsApp Bridge Server (Baileys)
 *
 * WARNING: This uses reverse-engineered WhatsApp Web protocol.
 * - Account ban risk exists
 * - Violates Meta ToS
 * - Do NOT use with important accounts
 * - For business use, use WhatsApp Business API instead
 */

const { makeWASocket, useMultiFileAuthState, DisconnectReason } = require('@whiskeysockets/baileys');
const express = require('express');
const qrcode = require('qrcode-terminal');
const pino = require('pino');

const app = express();
app.use(express.json());

const logger = pino({ level: 'info' });

// Configuration
const PORT = process.env.WHATSAPP_BRIDGE_PORT || 3001;
const CRATOS_WEBHOOK_URL = process.env.CRATOS_WEBHOOK_URL || 'http://localhost:3000/webhook/whatsapp';
const AUTH_DIR = process.env.WHATSAPP_AUTH_DIR || './auth';

let sock = null;
let qrData = null;
let connectionStatus = 'disconnected';

// Print warning on startup
console.log('\n');
console.log('========================================');
console.log('   WHATSAPP BRIDGE - IMPORTANT WARNING');
console.log('========================================');
console.log('This bridge uses Baileys (unofficial WhatsApp Web library).');
console.log('');
console.log('RISKS:');
console.log('  - Account BAN risk exists');
console.log('  - Violates Meta Terms of Service');
console.log('  - WhatsApp may block your number');
console.log('');
console.log('RECOMMENDATIONS:');
console.log('  - Do NOT use your primary phone number');
console.log('  - Use a secondary/disposable number');
console.log('  - For business: use WhatsApp Business API instead');
console.log('========================================');
console.log('\n');

/**
 * Initialize WhatsApp connection
 */
async function connectToWhatsApp() {
    const { state, saveCreds } = await useMultiFileAuthState(AUTH_DIR);

    sock = makeWASocket({
        auth: state,
        printQRInTerminal: true,
        logger: pino({ level: 'silent' }),
    });

    sock.ev.on('creds.update', saveCreds);

    sock.ev.on('connection.update', async (update) => {
        const { connection, lastDisconnect, qr } = update;

        if (qr) {
            qrData = qr;
            connectionStatus = 'waiting_scan';
            logger.info('QR code generated - scan with WhatsApp');
            qrcode.generate(qr, { small: true });
        }

        if (connection === 'close') {
            const shouldReconnect = lastDisconnect?.error?.output?.statusCode !== DisconnectReason.loggedOut;
            logger.info(`Connection closed: ${lastDisconnect?.error?.message}`);
            connectionStatus = 'disconnected';

            if (shouldReconnect) {
                logger.info('Reconnecting...');
                setTimeout(connectToWhatsApp, 3000);
            } else {
                logger.warn('Logged out - delete auth folder and restart to reconnect');
            }
        }

        if (connection === 'open') {
            qrData = null;
            connectionStatus = 'connected';
            logger.info('WhatsApp connection established');
        }
    });

    // Handle incoming messages
    sock.ev.on('messages.upsert', async ({ messages, type }) => {
        if (type !== 'notify') return;

        for (const msg of messages) {
            // Skip status broadcasts and own messages
            if (msg.key.remoteJid === 'status@broadcast') continue;
            if (msg.key.fromMe) continue;

            const messageContent = msg.message?.conversation
                || msg.message?.extendedTextMessage?.text
                || '';

            if (!messageContent) continue;

            const payload = {
                id: msg.key.id,
                from: msg.key.remoteJid,
                participant: msg.key.participant, // For groups
                text: messageContent,
                timestamp: msg.messageTimestamp,
                isGroup: msg.key.remoteJid.endsWith('@g.us'),
            };

            logger.info({ from: payload.from }, 'Received message');

            // Forward to Cratos webhook
            try {
                const response = await fetch(CRATOS_WEBHOOK_URL, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(payload),
                });

                if (!response.ok) {
                    logger.error({ status: response.status }, 'Failed to forward message to Cratos');
                }
            } catch (error) {
                logger.error({ error: error.message }, 'Failed to connect to Cratos webhook');
            }
        }
    });
}

// API Endpoints

/**
 * GET /status - Check connection status
 */
app.get('/status', (req, res) => {
    res.json({
        status: connectionStatus,
        qr: qrData,
        connected: connectionStatus === 'connected',
    });
});

/**
 * POST /connect - Start connection (returns QR if needed)
 */
app.post('/connect', async (req, res) => {
    if (connectionStatus === 'connected') {
        return res.json({ status: 'connected' });
    }

    if (!sock) {
        await connectToWhatsApp();
    }

    // Wait a bit for QR to generate
    await new Promise(resolve => setTimeout(resolve, 2000));

    res.json({
        status: connectionStatus,
        qr: qrData,
    });
});

/**
 * POST /disconnect - Disconnect (doesn't logout)
 */
app.post('/disconnect', async (req, res) => {
    if (sock) {
        sock.end();
        sock = null;
    }
    connectionStatus = 'disconnected';
    res.json({ status: 'disconnected' });
});

/**
 * POST /send - Send a message
 */
app.post('/send', async (req, res) => {
    const { to, message, quotedId } = req.body;

    if (!sock || connectionStatus !== 'connected') {
        return res.status(503).json({ error: 'Not connected to WhatsApp' });
    }

    if (!to || !message) {
        return res.status(400).json({ error: 'Missing "to" or "message" field' });
    }

    try {
        // Ensure proper JID format
        const jid = to.includes('@') ? to : `${to}@s.whatsapp.net`;

        const options = {};
        if (quotedId) {
            options.quoted = { key: { id: quotedId, remoteJid: jid } };
        }

        const sent = await sock.sendMessage(jid, { text: message }, options);

        res.json({
            success: true,
            messageId: sent.key.id,
        });
    } catch (error) {
        logger.error({ error: error.message }, 'Failed to send message');
        res.status(500).json({ error: error.message });
    }
});

/**
 * POST /typing - Send typing indicator
 */
app.post('/typing', async (req, res) => {
    const { to } = req.body;

    if (!sock || connectionStatus !== 'connected') {
        return res.status(503).json({ error: 'Not connected to WhatsApp' });
    }

    try {
        const jid = to.includes('@') ? to : `${to}@s.whatsapp.net`;
        await sock.sendPresenceUpdate('composing', jid);
        res.json({ success: true });
    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

/**
 * POST /read - Mark messages as read
 */
app.post('/read', async (req, res) => {
    const { to, messageIds } = req.body;

    if (!sock || connectionStatus !== 'connected') {
        return res.status(503).json({ error: 'Not connected to WhatsApp' });
    }

    try {
        const jid = to.includes('@') ? to : `${to}@s.whatsapp.net`;
        const keys = messageIds.map(id => ({ remoteJid: jid, id }));
        await sock.readMessages(keys);
        res.json({ success: true });
    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

// Health check
app.get('/health', (req, res) => {
    res.json({ ok: true, status: connectionStatus });
});

// Start server
app.listen(PORT, () => {
    logger.info(`WhatsApp Bridge listening on port ${PORT}`);
    logger.info(`Cratos webhook URL: ${CRATOS_WEBHOOK_URL}`);

    // Auto-connect on startup
    connectToWhatsApp().catch(err => {
        logger.error({ error: err.message }, 'Failed to auto-connect');
    });
});

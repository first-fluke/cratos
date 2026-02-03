# Cratos WhatsApp Bridge (Baileys)

WhatsApp bridge for Cratos using the unofficial Baileys library.

## Important Warning

This bridge uses reverse-engineered WhatsApp Web protocol (Baileys).

**RISKS:**
- Account **BAN** risk exists
- Violates Meta Terms of Service
- WhatsApp may permanently block your number

**RECOMMENDATIONS:**
- Do NOT use your primary phone number
- Use a secondary/disposable number
- For business use: use the official WhatsApp Business API instead

## Setup

```bash
# Install dependencies
npm install

# Start the server
npm start
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WHATSAPP_BRIDGE_PORT` | 3001 | Server port |
| `CRATOS_WEBHOOK_URL` | http://localhost:3000/webhook/whatsapp | Cratos webhook URL |
| `WHATSAPP_AUTH_DIR` | ./auth | Directory for auth state |

## API Endpoints

### GET /status
Check connection status and get QR code if available.

### POST /connect
Start WhatsApp connection. Returns QR code if not authenticated.

### POST /send
Send a message.
```json
{
  "to": "+821012345678",
  "message": "Hello!",
  "quotedId": "optional-message-id-to-reply"
}
```

### POST /typing
Send typing indicator.
```json
{
  "to": "+821012345678"
}
```

### POST /read
Mark messages as read.
```json
{
  "to": "+821012345678",
  "messageIds": ["id1", "id2"]
}
```

## Docker

```bash
# Build
docker build -t cratos-whatsapp-bridge .

# Run (with volume for auth persistence)
docker run -d \
  -p 3001:3001 \
  -v ./auth:/app/auth \
  -e CRATOS_WEBHOOK_URL=http://host.docker.internal:3000/webhook/whatsapp \
  cratos-whatsapp-bridge
```

## First Time Setup

1. Start the bridge: `npm start`
2. Scan the QR code with WhatsApp (Linked Devices > Link a Device)
3. Auth state is saved in `./auth` directory
4. Subsequent restarts will auto-connect

## Troubleshooting

### "Not connected to WhatsApp"
- Delete the `./auth` folder and restart
- Scan the QR code again

### Messages not being received
- Check that `CRATOS_WEBHOOK_URL` is correct
- Ensure Cratos is running and accepting webhooks

### Account banned
- Unfortunately, there's no recovery for banned accounts
- This is a known risk of using unofficial APIs
- Use a different number or switch to WhatsApp Business API

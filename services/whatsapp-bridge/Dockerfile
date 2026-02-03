FROM node:20-alpine

WORKDIR /app

# Copy package files
COPY package*.json ./

# Install dependencies
RUN npm ci --only=production

# Copy source
COPY . .

# Create auth directory
RUN mkdir -p /app/auth

# Environment variables
ENV WHATSAPP_BRIDGE_PORT=3001
ENV CRATOS_WEBHOOK_URL=http://host.docker.internal:3000/webhook/whatsapp
ENV WHATSAPP_AUTH_DIR=/app/auth

EXPOSE 3001

# Health check
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:3001/health || exit 1

CMD ["node", "server.js"]

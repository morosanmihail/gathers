FROM node:22-alpine AS builder

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY svelte.config.js tsconfig.json vite.config.ts version.js ./
COPY src ./src
# Favicon, PWA icons and manifest live here and are served from the site root.
COPY static ./static

# The build context has no .git, so the version (git tag) is passed in by CI.
ARG GATHERS_VERSION
ENV GATHERS_VERSION=$GATHERS_VERSION

RUN npm run build

FROM nginx:alpine

COPY --from=builder /app/build /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 3001

CMD ["nginx", "-g", "daemon off;"]

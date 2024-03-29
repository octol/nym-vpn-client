/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly APP_NOSPLASH: string | undefined;
  readonly APP_CREDENTIAL: string | undefined;
  readonly APP_SENTRY_DSN: string | undefined;
  readonly APP_PROTO_PATH: string | undefined;
  readonly APP_GRPC_URL: string | undefined;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

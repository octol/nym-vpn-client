// Original file: proto/vpn.proto

export const ConnectionStatus = {
  STATUS_UNSPECIFIED: 'STATUS_UNSPECIFIED',
  CONNECTED: 'CONNECTED',
  DISCONNECTED: 'DISCONNECTED',
  CONNECTING: 'CONNECTING',
  DISCONNECTING: 'DISCONNECTING',
  UNKNOWN: 'UNKNOWN',
} as const;

export type ConnectionStatus =
  | 'STATUS_UNSPECIFIED'
  | 0
  | 'CONNECTED'
  | 1
  | 'DISCONNECTED'
  | 2
  | 'CONNECTING'
  | 3
  | 'DISCONNECTING'
  | 4
  | 'UNKNOWN'
  | 5

export type ConnectionStatus__Output = typeof ConnectionStatus[keyof typeof ConnectionStatus]

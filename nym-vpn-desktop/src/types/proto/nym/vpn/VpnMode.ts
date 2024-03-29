// Original file: proto/vpn.proto

export const VpnMode = {
  MODE_UNSPECIFIED: 'MODE_UNSPECIFIED',
  MIXNET_FIVE_HOP: 'MIXNET_FIVE_HOP',
  MIXNET_TWO_HOP: 'MIXNET_TWO_HOP',
  WIREGUARD_TWO_HOP: 'WIREGUARD_TWO_HOP',
} as const;

export type VpnMode =
  | 'MODE_UNSPECIFIED'
  | 0
  | 'MIXNET_FIVE_HOP'
  | 1
  | 'MIXNET_TWO_HOP'
  | 2
  | 'WIREGUARD_TWO_HOP'
  | 3

export type VpnMode__Output = typeof VpnMode[keyof typeof VpnMode]

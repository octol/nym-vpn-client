import * as grpc from '@grpc/grpc-js';
import nymVpnPackage from './loader';

export const grpcClient = () => {
  const url = import.meta.env.APP_GRPC_URL;
  if (!url) {
    console.warn('APP_GRPC_URL not set in environment variables');
    return;
  }

  return new nymVpnPackage.NymVpnService(
    url,
    grpc.credentials.createInsecure(),
  );
};

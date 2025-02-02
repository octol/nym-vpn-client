import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import clsx from 'clsx';
import { useTranslation } from 'react-i18next';
import { Button } from '@headlessui/react';
import { type } from '@tauri-apps/plugin-os';
import { useInAppNotify, useMainDispatch, useMainState } from '../../contexts';
import { StateDispatch, VpnMode } from '../../types';
import { RadioGroup, RadioGroupOption } from '../../ui';
import { useThrottle } from '../../hooks';
import { HomeThrottleDelay } from '../../constants';
import MsIcon from '../../ui/MsIcon';
import ModeDetailsDialog from './ModeDetailsDialog';
import { S_STATE } from '../../static';

function NetworkModeSelect() {
  const state = useMainState();
  const dispatch = useMainDispatch() as StateDispatch;
  const [isDialogModesOpen, setIsDialogModesOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const { push } = useInAppNotify();
  const os = type();

  const { t } = useTranslation('home');

  useEffect(() => {
    if (state.vpnMode === 'TwoHop' && os === 'windows') {
      dispatch({ type: 'set-vpn-mode', mode: 'Mixnet' });
    }
  }, [os, dispatch, state.vpnMode]);

  const handleNetworkModeChange = async (value: VpnMode) => {
    if (state.state === 'Disconnected' && value !== state.vpnMode) {
      setLoading(true);
      try {
        await invoke<void>('set_vpn_mode', { mode: value });
        dispatch({ type: 'set-vpn-mode', mode: value });
        console.info('vpn mode set to', value);
      } catch (e) {
        console.warn(e);
      } finally {
        setLoading(false);
      }
    }
  };

  const showSnackbar = useThrottle(
    () => {
      let text = null;
      switch (state.state) {
        case 'Connected':
          text = t('snackbar-disabled-message.connected');
          break;
        case 'Connecting':
          text = t('snackbar-disabled-message.connecting');
          break;
        case 'Disconnecting':
          text = t('snackbar-disabled-message.disconnecting');
          break;
      }
      if (!text) {
        return;
      }
      push({
        text,
        position: 'top',
      });
    },
    HomeThrottleDelay,
    [state.state],
  );

  const handleDisabledState = () => {
    if (state.state !== 'Disconnected') {
      showSnackbar();
    }
  };

  const vpnModes = useMemo<RadioGroupOption<VpnMode>[]>(() => {
    return [
      {
        key: 'Mixnet',
        label: t('privacy-mode.title'),
        desc: t('privacy-mode.desc'),
        disabled: state.state !== 'Disconnected' || loading,
        icon: (
          <span className="font-icon text-3xl text-baltic-sea dark:text-mercury-pinkish">
            visibility_off
          </span>
        ),
      },
      {
        key: 'TwoHop',
        label: t('fast-mode.title'),
        desc: t('fast-mode.desc'),
        disabled:
          // TODO remove os check when Windows is supported
          os === 'windows' || state.state !== 'Disconnected' || loading,
        icon: (
          <span className="font-icon text-3xl text-baltic-sea dark:text-mercury-pinkish">
            speed
          </span>
        ),
        // TODO remove when Windows is supported
        className: os === 'windows' ? 'opacity-40' : undefined,
        tooltip: os === 'windows' ? t('windows-no-fast-mode') : undefined,
      },
    ];
  }, [os, loading, state.state, t]);

  return (
    <div>
      <div
        className={clsx([
          'flex flex-row items-center justify-between',
          'font-semibold text-base text-baltic-sea dark:text-white mb-5 cursor-default',
        ])}
      >
        <label>{t('select-mode-label')}</label>
        <Button
          className="w-6 focus:outline-none cursor-default"
          onClick={() => setIsDialogModesOpen(true)}
        >
          <MsIcon
            icon="info"
            className={clsx([
              'text-xl',
              'text-cement-feet dark:text-mercury-mist transition duration-150',
              'opacity-90 dark:opacity-100 hover:opacity-100 hover:text-gun-powder hover:dark:text-mercury-pinkish',
            ])}
          />
        </Button>
      </div>
      <ModeDetailsDialog
        isOpen={isDialogModesOpen}
        onClose={() => setIsDialogModesOpen(false)}
      />
      <div className="select-none" onClick={handleDisabledState}>
        <RadioGroup
          key={`_${S_STATE.vpnModeInit}`}
          defaultValue={state.vpnMode}
          options={vpnModes}
          onChange={handleNetworkModeChange}
          radioIcons={false}
        />
      </div>
    </div>
  );
}

export default NetworkModeSelect;

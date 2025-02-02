import { ReactNode } from 'react';
import clsx from 'clsx';
import { Button as HuButton } from '@headlessui/react';
import { type } from '@tauri-apps/plugin-os';

type ButtonProps = {
  children: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  color?: 'melon' | 'cornflower' | 'grey';
  outline?: boolean;
  className?: string;
  loading?: boolean;
};

function Spinner() {
  const os = type();

  return (
    <span
      className={clsx([
        'loader',
        os === 'linux' ? 'h-[28px] w-[28px]' : 'h-[22px] w-[22px] border-4',
        'border:white dark:border-[#2c2b2e] border-b-transparent dark:border-b-transparent',
      ])}
    ></span>
  );
}

function Button({
  onClick,
  children,
  disabled,
  color = 'melon',
  outline,
  className,
  loading,
}: ButtonProps) {
  const getColorStyle = () => {
    switch (color) {
      case 'melon':
        if (outline) {
          return 'border border-melon outline-melon';
        } else {
          return 'bg-melon';
        }
      case 'grey':
        return 'bg-dim-gray dark:bg-dusty-grey';
      case 'cornflower':
        return 'bg-cornflower';
    }
  };

  return (
    <HuButton
      className={clsx([
        'flex justify-center items-center w-full',
        'rounded-lg text-lg font-bold py-3 px-6',
        'text-white dark:text-baltic-sea',
        'focus:outline-none data-[focus]:ring-2 data-[focus]:ring-black data-[focus]:dark:ring-white',
        'transition data-[disabled]:opacity-60 data-[active]:ring-0',
        outline
          ? 'data-[hover]:ring-1 data-[hover]:ring-melon'
          : 'data-[hover]:opacity-80',
        'shadow tracking-normal cursor-default',
        getColorStyle(),
        className && className,
      ])}
      onClick={onClick}
      disabled={disabled}
    >
      {loading ? (
        Spinner()
      ) : (
        <div className={clsx(outline && `text-${color}`, 'truncate')}>
          {children}
        </div>
      )}
    </HuButton>
  );
}

export default Button;

import { Globe2 } from 'lucide-react';
import { useState } from 'react';
import { CHAIN_LOGOS } from '../lib/logos';

interface ChainIconProps {
  chain: string;
  className?: string;
}

export function ChainIcon({ chain, className = 'w-6 h-6' }: ChainIconProps) {
  const [failed, setFailed] = useState(false);
  const logoUrl = CHAIN_LOGOS[chain.toLowerCase()];
  
  if (!logoUrl || failed) {
    return <Globe2 className={className} />;
  }
  
  return (
    <img
      src={logoUrl}
      alt={chain}
      className={className}
      loading="lazy"
      onError={() => setFailed(true)}
      style={{ objectFit: 'contain' }}
    />
  );
}

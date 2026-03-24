import { Coins } from 'lucide-react';
import { useState } from 'react';
import { PROTOCOL_LOGOS } from '../lib/logos';

interface ProtocolIconProps {
  protocol: string;
  className?: string;
}

export function ProtocolIcon({ protocol, className = 'w-6 h-6' }: ProtocolIconProps) {
  const [failed, setFailed] = useState(false);
  const logoUrl = PROTOCOL_LOGOS[protocol.toLowerCase()];
  
  if (!logoUrl || failed) {
    return <Coins className={className} />;
  }
  
  return (
    <img
      src={logoUrl}
      alt={protocol}
      className={className}
      loading="lazy"
      onError={(e) => {
        console.error(`Failed to load logo for ${protocol}:`, logoUrl);
        setFailed(true);
      }}
      onLoad={() => {
        console.log(`Successfully loaded logo for ${protocol}:`, logoUrl);
      }}
      style={{ objectFit: 'contain' }}
    />
  );
}

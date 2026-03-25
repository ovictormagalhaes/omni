interface LogoProps {
  size?: 'small' | 'medium' | 'large'
  iconOnly?: boolean
  className?: string
}

export default function Logo({ size = 'medium', iconOnly = false, className = '' }: LogoProps) {
  const sizeMap = {
    small: { width: 32, height: 32, text: 'text-xl' },
    medium: { width: 48, height: 48, text: 'text-2xl' },
    large: { width: 64, height: 64, text: 'text-3xl' },
  }

  const { width, height, text } = sizeMap[size]

  if (iconOnly) {
    return (
      <img
        src="/logo-icon.svg"
        alt="OMNI"
        width={width}
        height={height}
        className={className}
      />
    )
  }

  return (
    <div className={`flex items-center gap-2.5 ${className}`}>
      <img
        src="/logo-icon.svg"
        alt="OMNI"
        width={width}
        height={height}
      />
      <span className={`${text} font-bold bg-gradient-to-r from-white via-slate-200 to-slate-400 bg-clip-text text-transparent`}>
        OMNI
      </span>
    </div>
  )
}

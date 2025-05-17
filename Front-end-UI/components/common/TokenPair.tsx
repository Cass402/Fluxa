import { Token } from "@/lib/types";

interface TokenPairProps {
  token1: Token;
  token2: Token;
  size?: "sm" | "md" | "lg";
}

export default function TokenPair({ token1, token2, size = "md" }: TokenPairProps) {
  // Determine size classes
  const sizeClasses = {
    sm: {
      container: "flex -space-x-1.5",
      image: "w-5 h-5 rounded-full border-2",
    },
    md: {
      container: "flex -space-x-2",
      image: "w-6 h-6 rounded-full border-2",
    },
    lg: {
      container: "flex -space-x-3",
      image: "w-8 h-8 rounded-full border-2",
    },
  };
  
  const { container, image } = sizeClasses[size];
  
  return (
    <div className={container}>
      <img
        src={token1.logo}
        alt={token1.symbol}
        className={`${image} border-background z-10`}
      />
      <img
        src={token2.logo}
        alt={token2.symbol}
        className={`${image} border-background`}
      />
    </div>
  );
}
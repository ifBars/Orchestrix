import { motion } from "framer-motion";

export function TypingCursor() {
  return (
    <motion.span
      className="inline h-[1.1em] w-[2px] bg-current align-text-bottom ml-[1px]"
      animate={{ opacity: [1, 0] }}
      transition={{
        duration: 0.5,
        repeat: Infinity,
        repeatType: "reverse",
        ease: "easeInOut",
      }}
    />
  );
}

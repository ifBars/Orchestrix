import { ArrowRight, Download, Github } from "lucide-react";
import { motion, type Variants } from "framer-motion";
import { PreviewWorkbench } from "@/components/landing/preview/PreviewWorkbench";
import { ORCHESTRIX_REPO_URL, ORCHESTRIX_RELEASES_URL } from "@/components/landing/constants";

const cinematicEase = [0.22, 1, 0.36, 1] as const;

const fadeUp: Variants = {
  hidden: { opacity: 0, y: 24 },
  visible: {
    opacity: 1,
    y: 0,
    transition: { duration: 0.72, ease: cinematicEase },
  },
};

const stagger: Variants = {
  hidden: {},
  visible: {
    transition: {
      staggerChildren: 0.08,
      delayChildren: 0.08,
    },
  },
};

const heroProof = [
  {
    label: "Plan",
    body: "inspect before build",
  },
  {
    label: "Review",
    body: "approve artifacts explicitly",
  },
  {
    label: "Events",
    body: "replay every transition",
  },
];

function BinaryBackdrop() {
  const rows = Array.from({ length: 8 }, (_, index) => {
    const chunk = index % 2 === 0 ? "010011010110010101101110" : "001101010100111101110010";
    return Array.from({ length: 7 }, () => chunk).join("  ");
  });

  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden opacity-30" aria-hidden="true">
      <div
        className="absolute inset-0"
        style={{
          background:
            "radial-gradient(circle at 14% 12%, oklch(0.72 0.12 240 / 0.1), transparent 26%), radial-gradient(circle at 84% 8%, oklch(0.78 0.09 70 / 0.08), transparent 16%)",
        }}
      />
      <div className="absolute inset-x-0 top-16 grid gap-6 px-6 text-[10px] font-medium tracking-[0.28em] text-primary/15 sm:px-12">
        {rows.map((row, index) => (
          <span
            key={index}
            className={
              index % 3 === 0 ? "translate-x-10" : index % 3 === 1 ? "-translate-x-3" : "translate-x-20"
            }
          >
            {row}
          </span>
        ))}
      </div>
    </div>
  );
}

export default function HeroSection() {
  return (
    <section id="top" className="landing-hero-section relative overflow-hidden">
      <BinaryBackdrop />

      <div className="pointer-events-none absolute inset-x-0 top-0 h-[420px] bg-[radial-gradient(circle_at_76%_14%,oklch(0.76_0.09_68_/_0.12),transparent_18%),radial-gradient(circle_at_82%_18%,oklch(0.72_0.12_240_/_0.16),transparent_34%),linear-gradient(180deg,transparent,oklch(0.13_0.012_260_/_0.56))]" />

      <div className="landing-hero-inner mx-auto w-full max-w-[1400px] px-6">
        <div className="landing-hero-grid">
          <motion.div
            className="landing-hero-copy"
            variants={stagger}
            initial="hidden"
            animate="visible"
          >
            <motion.h1
              variants={fadeUp}
              className="max-w-[11ch] text-5xl font-semibold leading-[0.94] tracking-[-0.06em] text-foreground sm:text-6xl lg:text-[4.35rem] xl:text-[4.75rem]"
            >
              The review-first agent workspace for real codebases.
            </motion.h1>

            <motion.p
              variants={fadeUp}
              className="landing-hero-copy__lead mt-5 max-w-xl text-base leading-8 text-muted-foreground sm:text-lg"
            >
              Orchestrix gives you IDE-grade chrome, explicit approval checkpoints, and event-level visibility so
              complex work stays inspectable instead of magical.
            </motion.p>

            <motion.div variants={fadeUp} className="landing-hero-actions mt-7 flex flex-col gap-3 sm:flex-row">
              <a
                href={ORCHESTRIX_RELEASES_URL}
                target="_blank"
                rel="noreferrer"
                className="inline-flex h-12 items-center justify-center gap-2 rounded-full bg-primary px-6 text-sm font-medium text-primary-foreground transition-transform duration-200 hover:-translate-y-0.5"
              >
                <Download size={16} />
                Download alpha
              </a>
              <a
                href="#preview"
                className="inline-flex h-12 items-center justify-center gap-2 rounded-full border border-border/70 bg-card/65 px-6 text-sm font-medium text-foreground transition-colors hover:bg-accent/60"
              >
                Explore preview
                <ArrowRight size={15} />
              </a>
              <a
                href={ORCHESTRIX_REPO_URL}
                target="_blank"
                rel="noreferrer"
                className="inline-flex h-12 items-center justify-center gap-2 rounded-full border border-border/70 px-6 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
              >
                <Github size={15} />
                GitHub
              </a>
            </motion.div>

            <motion.div variants={fadeUp} className="landing-hero-proof-grid mt-8">
              {heroProof.map((item) => (
                <article key={item.label} className="landing-hero-proof">
                  <p className="landing-hero-proof__label">{item.label}</p>
                  <p className="landing-hero-proof__body">{item.body}</p>
                </article>
              ))}
            </motion.div>
          </motion.div>

          <motion.div
            className="landing-hero-stage"
            initial={{ opacity: 0, y: 24 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.8, delay: 0.12, ease: cinematicEase }}
          >
            <div className="landing-hero-stage__intro">
              <p className="landing-hero-stage__eyebrow">Local product preview</p>
              <h2 className="landing-hero-stage__title">Timeline, review, and artifacts in one calm shell.</h2>
            </div>

            <PreviewWorkbench variant="hero" interactive={false} initialScenario="executing" />
          </motion.div>
        </div>
      </div>
    </section>
  );
}

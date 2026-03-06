import {
  CTASection,
  Footer,
  HeroSection,
  Navbar,
  PreviewSection,
  ProofStripSection,
  TechnicalProofSection,
  WorkflowSection,
} from "@/components/landing";

const LandingPage = () => {
  return (
    <div className="landing-page min-h-screen bg-background text-foreground">
      <Navbar />
      <main>
        <HeroSection />
        <ProofStripSection />
        <PreviewSection />
        <WorkflowSection />
        <TechnicalProofSection />
        <CTASection />
      </main>
      <Footer />
    </div>
  );
};

export default LandingPage;


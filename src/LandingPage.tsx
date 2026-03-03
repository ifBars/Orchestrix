import {
  Navbar,
  HeroSection,
  ExecutionModelSection,
  ArchitectureAndVisibilitySection,
  AgentsAndToolsSection,
  ProvidersAndRecoverySection,
  CTASection,
  Footer,
} from "@/components/landing";

const LandingPage = () => {
  return (
    <div className="min-h-screen bg-background">
      <Navbar />
      <main>
        <HeroSection />
        <ExecutionModelSection />
        <ArchitectureAndVisibilitySection />
        <AgentsAndToolsSection />
        <ProvidersAndRecoverySection />
        <CTASection />
      </main>
      <Footer />
    </div>
  );
};

export default LandingPage;

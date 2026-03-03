import {
  Navbar,
  HeroSection,
  ExecutionModelSection,
  EventArchitectureSection,
  VisibilitySection,
  SubAgentsSection,
  ToolSystemSection,
  ProvidersSection,
  CrashRecoverySection,
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
        <EventArchitectureSection />
        <VisibilitySection />
        <SubAgentsSection />
        <ToolSystemSection />
        <ProvidersSection />
        <CrashRecoverySection />
        <CTASection />
      </main>
      <Footer />
    </div>
  );
};

export default LandingPage;

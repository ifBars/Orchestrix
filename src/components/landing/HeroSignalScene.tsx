import { useEffect, useRef } from "react";
import * as THREE from "three";

export function HeroSignalScene() {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(38, 1, 0.1, 100);
    camera.position.set(0, 0, 8);

    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));
    renderer.setClearColor(0x000000, 0);
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.domElement.className = "landing-signal-scene__canvas";
    container.appendChild(renderer.domElement);

    const root = new THREE.Group();
    scene.add(root);

    const ambientLight = new THREE.AmbientLight(0x8db2ff, 1.15);
    scene.add(ambientLight);

    const coolLight = new THREE.PointLight(0x63a7ff, 16, 24, 2);
    coolLight.position.set(3.5, 2.2, 5.5);
    scene.add(coolLight);

    const emberLight = new THREE.PointLight(0xff9c6a, 9, 20, 2);
    emberLight.position.set(-3, -1.5, 4);
    scene.add(emberLight);

    const shellGeometry = new THREE.IcosahedronGeometry(1.45, 1);
    const shellMaterial = new THREE.MeshStandardMaterial({
      color: 0x8ab3ff,
      emissive: 0x183a6f,
      emissiveIntensity: 1.1,
      roughness: 0.28,
      metalness: 0.22,
      wireframe: true,
      transparent: true,
      opacity: 0.34,
    });
    const shellMesh = new THREE.Mesh(shellGeometry, shellMaterial);
    root.add(shellMesh);

    const coreGeometry = new THREE.OctahedronGeometry(0.62, 0);
    const coreMaterial = new THREE.MeshStandardMaterial({
      color: 0xf3f7ff,
      emissive: 0x8ab3ff,
      emissiveIntensity: 1.9,
      roughness: 0.16,
      metalness: 0.08,
      transparent: true,
      opacity: 0.95,
    });
    const coreMesh = new THREE.Mesh(coreGeometry, coreMaterial);
    root.add(coreMesh);

    const ringMaterial = new THREE.MeshBasicMaterial({
      color: 0x74aaff,
      transparent: true,
      opacity: 0.58,
    });
    const ring = new THREE.Mesh(new THREE.TorusGeometry(2.05, 0.03, 12, 160), ringMaterial);
    ring.rotation.x = Math.PI / 2.35;
    ring.rotation.y = Math.PI / 8;
    root.add(ring);

    const emberRingMaterial = new THREE.MeshBasicMaterial({
      color: 0xff9466,
      transparent: true,
      opacity: 0.28,
    });
    const emberRing = new THREE.Mesh(
      new THREE.TorusGeometry(2.55, 0.024, 12, 180),
      emberRingMaterial
    );
    emberRing.rotation.x = Math.PI / 2.05;
    emberRing.rotation.z = Math.PI / 4.8;
    root.add(emberRing);

    const edgeGeometry = new THREE.EdgesGeometry(new THREE.IcosahedronGeometry(1.9, 1));
    const edgeMaterial = new THREE.LineBasicMaterial({
      color: 0x76a9ff,
      transparent: true,
      opacity: 0.18,
    });
    const edgeMesh = new THREE.LineSegments(edgeGeometry, edgeMaterial);
    root.add(edgeMesh);

    const particleCount = 160;
    const particlePositions = new Float32Array(particleCount * 3);
    for (let index = 0; index < particleCount; index += 1) {
      const stride = index * 3;
      const angle = (index / particleCount) * Math.PI * 2;
      const radius = 2.15 + Math.sin(index * 0.7) * 0.45 + Math.random() * 0.35;
      particlePositions[stride] = Math.cos(angle) * radius;
      particlePositions[stride + 1] = Math.sin(angle) * radius * 0.55;
      particlePositions[stride + 2] = (Math.random() - 0.5) * 2.6;
    }

    const particleGeometry = new THREE.BufferGeometry();
    particleGeometry.setAttribute("position", new THREE.BufferAttribute(particlePositions, 3));
    const particleMaterial = new THREE.PointsMaterial({
      color: 0x93bdff,
      size: 0.045,
      transparent: true,
      opacity: 0.92,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
    });
    const particleCloud = new THREE.Points(particleGeometry, particleMaterial);
    root.add(particleCloud);

    let width = 0;
    let height = 0;
    const resize = () => {
      width = Math.max(container.clientWidth, 1);
      height = Math.max(container.clientHeight, 1);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
      renderer.setSize(width, height);
    };

    const resizeObserver = new ResizeObserver(() => resize());
    resizeObserver.observe(container);
    resize();

    const clock = new THREE.Clock();
    let frameId = 0;

    const tick = () => {
      const elapsed = clock.getElapsedTime();
      shellMesh.rotation.x = elapsed * 0.22;
      shellMesh.rotation.y = elapsed * 0.3;
      coreMesh.rotation.x = -elapsed * 0.52;
      coreMesh.rotation.y = elapsed * 0.76;
      ring.rotation.z = elapsed * 0.14;
      emberRing.rotation.y = -elapsed * 0.1;
      edgeMesh.rotation.x = -elapsed * 0.16;
      edgeMesh.rotation.y = elapsed * 0.2;
      particleCloud.rotation.z = elapsed * 0.05;
      root.position.y = Math.sin(elapsed * 0.8) * 0.12;
      root.position.x = Math.cos(elapsed * 0.46) * 0.08;
      renderer.render(scene, camera);
      frameId = window.requestAnimationFrame(tick);
    };

    tick();

    return () => {
      window.cancelAnimationFrame(frameId);
      resizeObserver.disconnect();
      container.removeChild(renderer.domElement);
      shellGeometry.dispose();
      shellMaterial.dispose();
      coreGeometry.dispose();
      coreMaterial.dispose();
      ring.geometry.dispose();
      ringMaterial.dispose();
      emberRing.geometry.dispose();
      emberRingMaterial.dispose();
      edgeGeometry.dispose();
      edgeMaterial.dispose();
      particleGeometry.dispose();
      particleMaterial.dispose();
      renderer.dispose();
    };
  }, []);

  return <div ref={containerRef} className="landing-signal-scene" aria-hidden="true" />;
}

# AI Agent Harness, 3 Principles for Context Engineering, and the Bitter Lesson Revisited

### Be ready to rebuild or perish

[In a recent episode of High Signal](https://high-signal.delphina.ai/episode/context-engineering-to-ai-agent-harnesses-the-new-software-discipline), we spoke with Lance Martin, a machine learning engineer at LangChain, about the new engineering disciplines emerging in the era of generative AI. With a background building production ML systems at Uber and now working on tooling to help developers build, test, and deploy reliable AI agents at Langchain, Lance has a wonderful perspective on what has fundamentally changed and what principles endure.

This post captures some of our favourite parts of the conversation, including the

-   The _**shift from training models to orchestrating them**_,
    
-   the _**importance of agent harnesses**_ and _**re-architecting them as models improve**_,
    
-   _**3 key principles**_ for _**mastering context engineering**_,
    
-   Navigating the _**agentic spectrum**_: _**human supervision, feedback cycles**_, and _**risk tolerance.**_
    

[read the full show notes here](https://high-signal.delphina.ai/episode/context-engineering-to-ai-agent-harnesses-the-new-software-discipline).

### From Training to Orchestration: A New Era in AI Engineering

Three major shifts have reshaped the AI landscape over the past several years:

1.  **Architectural Consolidation**: The transformer architecture has become dominant, absorbing more specialized architectures like CNNs and RNNs. This, combined with scaling laws, has led to much larger and more general-purpose models.
    
2.  **Model APIs >> Training Models**: The industry has moved from a world where every company trained its own proprietary models to one where a few foundation model providers offer powerful primitives through APIs. This has flipped the ratio of model trainers to model users.
    
3.  **Higher Level of Abstraction For Builders**: As a result, the core engineering challenge has shifted. This new reality has given rise to a new set of engineering disciplines focused on **orchestration**: _**prompt engineering, context engineering,** and **building agents** on top of these **powerful new primitives.**_
    

### Applying Classic ML Principles to AI Systems

While the technology has changed, several core principles from traditional ML engineering not only apply but are more critical than ever.

-   **Simplicity Remains Essential**: It’s tempting to jump to complex solutions like agents, but starting with the simplest possible approach (often just thoughtful prompt engineering or a structured workflow) is crucial for success. Start simple and progressively add complexity only when necessary.
    
-   **Observability and Evaluation**: With non-deterministic systems, understanding _what_ is happening (tracing) and rigorously evaluating it is paramount. This requires a new kind of evaluation that goes beyond traditional unit tests to account for variability in LLM outputs.
    
-   **Verifier’s Law**: Lance recalls [an idea from Jason Wei](https://www.jasonwei.net/blog/asymmetry-of-verification-and-verifiers-law): the ability to train an AI for a task is proportional to how easily verifiable that task is. Establishing clear verification criteria is a foundational prerequisite for achieving high quality and a necessary step before attempting more advanced techniques like reinforcement fine-tuning.
    

### Agent Harness and the Application Layer: The Bitter Lesson Revisited

From Learning the Bitter Lesson

One of the most disorienting challenges of building with LLMs is that the underlying platform is improving exponentially. This brings Rich Sutton’s famous essay, [“The Bitter Lesson,”](http://www.incompleteideas.net/IncIdeas/BitterLesson.html) into sharp focus: _Sutton argues that general methods leveraging computation ultimately win out over handcrafted, complex systems_.

**This lesson now applies at the application layer:** the architectural assumptions baked into an application today will likely be obsolete in six months when a new, more capable model is released.

> [Over time models get better and you’re having to strip away structure, remove assumptions and make your harness or your system simpler and adapt to the models.](https://youtu.be/2Muxy3wE-E0?t=736)

This reality demands a new mindset:

-   **The “Agent Harness”**: This is the scaffolding around the LLM that manages tool execution, message history, and context. As models improve, this harness must be continually simplified, stripping away crutches that are no longer needed.
    
-   **Embrace Re-architecture**: Teams must be willing to constantly reassess and rebuild. The popular agent [Manus has been re-architected five times since March 2024](https://youtu.be/2Muxy3wE-E0?t=693), and [LangChain’s Open Deep Research](https://blog.langchain.com/open-deep-research/) was rebuilt multiple times in a year to keep pace with model improvements. Even Anthropic rips out Claude Code’s agent harness as models improve!
    

### Mastering Context Engineering: Reduce, Offload, Isolate

From Context Engineering for Agents

One of the most critical and overlooked disciplines, particularly with agentic systems, is **context engineering**.

Simply appending tool call results to a growing message list is expensive, slow, and degrades model performance. Even models with million-token context windows suffer from **“[context rot](https://research.trychroma.com/context-rot),”** where instruction-following ability diminishes as the context grows.

> [Often the effective context window for these LLMs is actually quite a bit lower than the stated technical one.](https://youtu.be/2Muxy3wE-E0?t=1836) _[So something to be very careful of.](https://youtu.be/2Muxy3wE-E0?t=1836)_

Lance outlines a three-part playbook used by leading agentic systems like Manus and Claude Code to manage context effectively:

1.  **Reduce**: Actively shrink the context passed to the model. This can be done by **compacting older tool calls** (keeping only a summary) or using **trajectory summarization** to compress the entire history once it reaches a certain size.
    
2.  **Offload**: Move information and complexity out of the prompt. This includes saving full tool results to an external file system for later reference. More profoundly, it means **offloading the action space**. Instead of giving an agent 100 different tools (which bloats the prompt), [give it a few atomic tools like a bash terminal](https://youtu.be/2Muxy3wE-E0?t=2033). This allows the agent to execute a vast range of commands without cluttering the context.
    
3.  **Isolate**: Use **multi-agent architectures** to delegate token-heavy sub-tasks. A main agent can offload a complex job to a specialized sub-agent, which performs the work in its own isolated context and returns only a concise result.
    
From Context Engineering for Agents

**Evaluation practices are also evolving rapidly**. Static benchmarks become saturated quickly, so the most effective teams rely on a more dynamic approach.

-   **Dogfooding and User Feedback**: The primary sources of evaluation data for products like Claude Code and Manus are [aggressive internal dogfooding and direct in-app user feedback](https://youtu.be/2Muxy3wE-E0?t=2484). Capturing real-world failure cases is key.
    
-   **Component-Level Evals**: It’s beneficial to set up separate evaluations for individual components of a system, such as the retrieval step in a RAG pipeline, to isolate and fix issues.
    
-   **Future-Proofing**: Test your system against models of varying capabilities. If performance scales up with more powerful models, your harness is likely well-designed and not a bottleneck.
    

### Workflows vs. Agents: A Spectrum of Autonomy

From Stop Building AI Agents... And Start Building Smarter AI Workflows

It’s also important to make clear that full-blown agents are not always the solution! A common point of confusion is when to use a structured workflow versus a more autonomous agent. [The key distinction is autonomy](https://youtu.be/2Muxy3wE-E0?t=985).

-   **Workflows** are systems with predefined, predictable steps. An LLM call can be one step in a fixed sequence (A → B → C). This is ideal for tasks with a known structure, like running a test suite or migrating a legacy codebase. Frameworks like LangChain’s **LangGraph** are designed for building these.
    
-   **Agents** are systems where the LLM dynamically chooses its own tools and processes to solve a problem. They are best suited for open-ended, adaptive tasks like research or complex coding, where the path to a solution is not known in advance.
    

This is not a binary choice but a spectrum. You can have systems with varying degrees of agency, and it’s even common to embed an agent as one step within a larger workflow. As models become more reliable, we’re also seeing the rise of **background or “ambient” agents** that can perform long-horizon tasks asynchronously, such as managing an email inbox. [These systems require carefully designed human-in-the-loop checkpoints and memory to learn from feedback over time](https://youtu.be/2Muxy3wE-E0?t=1533).

Full-blown agentic systems thrive when there’s high supervision, rapid feedback loops, and low risk.

### Key Takeaways for AI and Engineering Leaders

Lance gave us five key principles for leaders navigating this new landscape:

1.  **Start Simple**: Exhaust prompt engineering and simple workflows before moving to agents. Consider fine-tuning only as a last resort.
    
2.  **Build for Change**: Accept that the “Bitter Lesson” is real. What you build today will need to be re-architected as models improve.
    
3.  **Don’t Fear Rebuilding**: The cost and time required to rebuild systems are dramatically lower now, thanks to powerful code-generation models.
    
4.  **Patience Pays Off**: An application that is not viable today may be unlocked by the next generation of models. The success of Cursor after the release of Claude 3.5 Sonnet is a prime example.
    
5.  **Be Wary of Premature Training**: Don’t rush to fine-tune. Frontier models often quickly acquire the capabilities that teams spend months building into custom models.
    

Building applications with generative AI is a fundamentally new engineering discipline. It rewards _**orchestration** over architecture, **adaptation** over rigidity, and **simplicity** over complexity_.

The challenge for technical leaders is not just to build systems that work today, but to foster a culture and technical practice that can evolve with the powerful, ever-improving models at their core.

## Resources

-   [Lance on LinkedIn](https://www.linkedin.com/in/lance-martin-64a33b5/)
    
-   [Context Engineering for Agents by Lance Martin](https://rlancemartin.github.io/2025/06/23/context_engineering/)
    
-   [Learning the Bitter Lesson by Lance Martin](https://rlancemartin.github.io/2025/07/30/bitter_lesson/)
    
-   [Context Engineering in Manus by Lance Martin](https://rlancemartin.github.io/2025/10/15/manus/)
    
-   [Context Rot: How Increasing Input Tokens Impacts LLM Performance by Chroma](https://research.trychroma.com/context-rot)
    
-   [Building effective agents by Erik Schluntz and Barry Zhang at Anthropic](https://www.anthropic.com/engineering/building-effective-agents)
    
-   [Effective context engineering for AI agents by Anthropic](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
    
-   [How we built our multi-agent research system by Anthropic](https://www.anthropic.com/engineering/multi-agent-research-system)
    
-   [Measuring AI Ability to Complete Long Tasks by METR](https://metr.org/blog/2025-03-19-measuring-ai-ability-to-complete-long-tasks/)
    
-   [Your AI Product Needs Evals by Hamel Husain](https://hamel.dev/blog/posts/evals/index.html)
    
-   [Introducing Roast: Structured AI workflows made easy by Shopify](https://shopify.engineering/introducing-roast)
    
-   [Watch the podcast episode on YouTube](https://youtu.be/2Muxy3wE-E0)
    
-   [Delphina’s Newsletter](https://delphinaai.substack.com/)
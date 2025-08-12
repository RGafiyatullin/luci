# Overview

The test framework models each scenario as a directed acyclic graph of events.
Each event is a node in this graph, with edges representing happens-after dependencies.
A node becomes ready once all its dependencies are complete; once ready, it never becomes unready.
When multiple nodes are ready, execution follows a fixed priority: binds -> send/respond -> recv/delay, breaking ties by declaration order.

Events can be one of several types:
- bind — checks or establishes variable values without performing I/O.
- send — enqueues a message from a dummy to an actor (or routed if no target specified).
- recv — consumes a message from a queue, optionally binding values from its payload.
- respond — replies to a previously received request, automatically routed to the original requester.
- delay — advances simulated time, optionally in steps to allow intermediate processing.
- call — invokes another subroutine (its own event graph) with isolated variable scope; values are passed explicitly in/out.

Actors are the real components under test.
Dummies are scripted participants driven entirely by the test engine.
Symbolic names are used for both; actual Elfo addresses are hidden and non-deterministic.

The scenario runs until there is nothing left to do — no ready events, no active delays, no queued messages eligible for delivery, and no calls with pending work.
At that point, the engine evaluates success or failure based on the require conditions on events.

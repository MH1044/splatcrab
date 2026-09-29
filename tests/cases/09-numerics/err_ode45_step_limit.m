% covers: 15 - (Scope, invariant 6) ode45 whose MaxStep of 1e-6 needs a million steps over [0 1] meets its step cap of 50,000: a clean error, exit 1, never a hang
% The text is SplatCrab's own, recorded in the spec's Design notes.
% y' = max(t, y) through a builtin's name, and Refine 1, keep the 50,000
% attempts near a second in the debug build.
[t, y] = ode45('max', [0 1], 0, odeset('MaxStep', 1e-6, 'Refine', 1));

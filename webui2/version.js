import { execSync } from 'node:child_process';

// GATHERS_VERSION (set by CI / the Docker build, which has no .git) wins,
// otherwise ask git, otherwise fall back to 'dev'.
function resolve() {
	const fromEnv = process.env.GATHERS_VERSION?.trim();
	if (fromEnv) return fromEnv;
	try {
		return execSync('git describe --tags --always --dirty', {
			stdio: ['ignore', 'pipe', 'ignore']
		})
			.toString()
			.trim();
	} catch {
		return 'dev';
	}
}

export const appVersion = resolve();

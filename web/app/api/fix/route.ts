import { NextResponse } from 'next/server';
import { exec } from 'child_process';
import path from 'path';
import util from 'util';

const execAsync = util.promisify(exec);

export async function POST() {
    try {
        // Resolve path to the python script
        // We are in web/app/api/fix/route.ts -> need to go up to workflows/fix_system.py
        // process.cwd() in Next.js usually points to the root of the next app (web/)
        const scriptPath = path.resolve(process.cwd(), '../workflows/fix_system.py');

        // Execute the script
        const { stdout, stderr } = await execAsync(`python3 "${scriptPath}"`);

        // The script prints log lines to stderr and JSON to stdout (hopefully).
        // Our fix_system.py currently prints:
        // stderr: "Running system health check..."
        // stdout: JSON health data (and also some text... wait, I need to check fix_system.py again)

        // Let's re-read the python script output logic.
        // In the last step, I added:
        // print("\n" + "="*40) ... text analysis
        // print(json.dumps(health_data, indent=2)) ... wait, I need to ensure it ONLY outputs valid JSON to stdout for easy parsing, 
        // OR I need to parse the JSON out of the text mixed output.

        // Simplest fix: Update fix_system.py to have a --json flag, or just use the JSON part.
        // For now, let's just return the raw stdout/stderr to the frontend and let the frontend parse or display it.

        return NextResponse.json({
            success: true,
            stdout,
            stderr
        });

    } catch (error: any) {
        return NextResponse.json({
            success: false,
            error: error.message,
            stderr: error.stderr
        }, { status: 500 });
    }
}

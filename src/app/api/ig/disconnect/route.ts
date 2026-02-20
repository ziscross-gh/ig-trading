import { NextResponse } from 'next/server';

// POST - Disconnect from IG
export async function POST() {
  // In a real app, you'd clear the session/token here
  return NextResponse.json({
    success: true,
    authenticated: false,
    message: 'Disconnected successfully'
  });
}

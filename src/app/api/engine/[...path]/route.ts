import { NextRequest, NextResponse } from 'next/server';

// ============================================
// Rust Engine Proxy
// ============================================
// Forwards requests from the dashboard to the Rust engine.
// This avoids CORS issues and keeps the engine port internal.

const ENGINE_URL = process.env.ENGINE_INTERNAL_URL || 'http://localhost:9090';

async function proxyToEngine(
  request: NextRequest,
  params: { path: string[] }
) {
  const path = params.path.join('/');
  const url = new URL(`/api/${path}`, ENGINE_URL);

  // Forward query params
  request.nextUrl.searchParams.forEach((value, key) => {
    url.searchParams.set(key, value);
  });

  try {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    const fetchOptions: RequestInit = {
      method: request.method,
      headers,
    };

    // Forward body for POST/PUT/PATCH
    if (['POST', 'PUT', 'PATCH'].includes(request.method)) {
      const body = await request.text();
      if (body) fetchOptions.body = body;
    }

    const response = await fetch(url.toString(), fetchOptions);
    const data = await response.json();

    return NextResponse.json(data, { status: response.status });
  } catch (error) {
    console.error(`Engine proxy error [${path}]:`, error);
    return NextResponse.json(
      {
        error: 'Engine unreachable',
        details: error instanceof Error ? error.message : 'Connection refused',
      },
      { status: 502 }
    );
  }
}

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> }
) {
  return proxyToEngine(request, await params);
}

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> }
) {
  return proxyToEngine(request, await params);
}

export async function PUT(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> }
) {
  return proxyToEngine(request, await params);
}

export async function DELETE(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> }
) {
  return proxyToEngine(request, await params);
}

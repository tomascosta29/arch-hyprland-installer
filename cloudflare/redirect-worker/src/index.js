export default {
  async fetch(request) {
    const url = new URL(request.url);
    const targetUrl = "https://raw.githubusercontent.com/tomascosta29/arch-hyprland-installer/quickshell/bootstrap.sh";

    if (url.searchParams.has("redirect")) {
      return Response.redirect(targetUrl, 302);
    }

    try {
      const response = await fetch(targetUrl);
      if (response.ok) {
        const scriptText = await response.text();
        return new Response(scriptText, {
          headers: {
            "content-type": "text/plain; charset=utf-8",
            "cache-control": "public, max-age=300",
          },
        });
      }
    } catch (e) {
      // Fallback redirect if fetch fails
    }

    return Response.redirect(targetUrl, 302);
  },
};

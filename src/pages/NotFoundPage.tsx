import { Link } from "react-router-dom";

import { TopBar } from "@/shell/TopBar";
import { Button } from "@/components/ui/button";

export function NotFoundPage() {
  return (
    <>
      <TopBar title="Not found" subtitle="The route you opened doesn't exist." />
      <div className="flex flex-1 items-center justify-center p-6">
        <div className="flex flex-col items-center gap-3 text-center">
          <div className="text-5xl font-bold text-ink-muted">404</div>
          <Button asChild>
            <Link to="/">Back to dashboard</Link>
          </Button>
        </div>
      </div>
    </>
  );
}

import { expect, test } from "vitest";

import { name } from "./index";

test("the package names itself", () => {
  expect(name).toBe("@planroom/contract");
});

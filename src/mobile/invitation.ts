export interface FragmentLocation {
  hash: string;
  pathname: string;
  search: string;
}

export function takeInvitation(
  location: FragmentLocation,
  replace: (url: string) => void,
): string | null {
  const params = new URLSearchParams(location.hash.replace(/^#/, ""));
  const invite = params.get("invite");
  if (location.hash) replace(`${location.pathname}${location.search}`);
  return invite && invite.length <= 512 ? invite : null;
}

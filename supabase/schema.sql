-- =============================================================================
-- Schafkopf Kasse - Supabase Auth + Row-Level Security (RLS) Schema
-- Run this script in the Supabase SQL Editor (Free Tier compatible).
-- =============================================================================

create extension if not exists pgcrypto;

-- 1. Normalized Tables
create table if not exists public.players (
    id uuid primary key default gen_random_uuid(),
    name text not null,
    avatar_color text not null default '#10b981',
    active boolean not null default true,
    claimed_by uuid unique references auth.users(id) on delete set null,
    is_admin boolean not null default false,
    created_at timestamptz not null default now()
);

create table if not exists public.sessions (
    id uuid primary key default gen_random_uuid(),
    played_at date not null default current_date,
    duration_minutes integer,
    note text not null default '',
    created_by uuid references auth.users(id) on delete set null default auth.uid(),
    created_at timestamptz not null default now()
);

create table if not exists public.session_results (
    session_id uuid not null references public.sessions(id) on delete cascade,
    player_id uuid not null references public.players(id) on delete cascade,
    amount_cents bigint not null default 0,
    updated_at timestamptz not null default now(),
    primary key (session_id, player_id)
);

-- Seed default players if table is empty (Korbinian is default Admin)
insert into public.players (name, avatar_color, is_admin)
select * from (values
    ('Korbinian', '#10b981', true),
    ('Max',       '#3b82f6', false),
    ('Lukas',     '#f59e0b', false),
    ('Sepp',      '#ec4899', false)
) as v(name, avatar_color, is_admin)
where not exists (select 1 from public.players);

-- 2. Helper Functions for RLS
create or replace function public.is_current_user_admin()
returns boolean
language sql
security definer
stable
set search_path = public
as $$
    select exists (
        select 1
        from public.players
        where claimed_by = auth.uid()
          and is_admin = true
    );
$$;

create or replace function public.current_user_player_id()
returns uuid
language sql
security definer
stable
set search_path = public
as $$
    select id
    from public.players
    where claimed_by = auth.uid()
    limit 1;
$$;

-- 3. Claim a Player Profile (RPC)
-- A logged-in user can claim an unclaimed player profile.
-- If no admin exists yet in the table, the first user to claim 'Korbinian' (or first claim overall) becomes admin.
create or replace function public.claim_player_profile(p_player_id uuid)
returns jsonb
language plpgsql
security definer
set search_path = public
as $$
declare
    v_uid uuid := auth.uid();
    v_target public.players%rowtype;
    v_any_admin boolean;
begin
    if v_uid is null then
        raise exception 'Du musst eingeloggt sein, um ein Spielerprofil auszuwählen.';
    end if;

    select * into v_target from public.players where id = p_player_id;
    if not found then
        raise exception 'Spieler nicht gefunden.';
    end if;

    if v_target.claimed_by is not null and v_target.claimed_by <> v_uid then
        raise exception 'Dieses Spielerprofil ist bereits mit einem anderen Account verknüpft.';
    end if;

    -- Release any previously claimed profile of this user (unless admin wants to switch)
    update public.players
    set claimed_by = null
    where claimed_by = v_uid
      and id <> p_player_id;

    -- Check if any claimed admin exists
    select exists(
        select 1 from public.players where is_admin = true and claimed_by is not null
    ) into v_any_admin;

    update public.players
    set claimed_by = v_uid,
        is_admin = case
            when is_admin then true
            when lower(name) = 'korbinian' then true
            when not v_any_admin then true
            else false
        end
    where id = p_player_id
    returning * into v_target;

    return to_jsonb(v_target);
end;
$$;

-- 4. Enable Row Level Security (RLS)
alter table public.players enable row level security;
alter table public.sessions enable row level security;
alter table public.session_results enable row level security;

-- Policies for `players`:
-- Only authenticated (logged-in) users can read players
drop policy if exists "players_select_all" on public.players;
create policy "players_select_all" on public.players
    for select to authenticated using (true);

-- Only Admin can insert or delete players; regular users can update their own claimed player (e.g. avatar_color)
drop policy if exists "players_insert_admin" on public.players;
create policy "players_insert_admin" on public.players
    for insert to authenticated
    with check (public.is_current_user_admin());

drop policy if exists "players_update_own_or_admin" on public.players;
create policy "players_update_own_or_admin" on public.players
    for update to authenticated
    using (claimed_by = auth.uid() or public.is_current_user_admin())
    with check (claimed_by = auth.uid() or public.is_current_user_admin());

drop policy if exists "players_delete_admin" on public.players;
create policy "players_delete_admin" on public.players
    for delete to authenticated
    using (public.is_current_user_admin());

-- Policies for `sessions`:
-- Only authenticated (logged-in) users can read sessions
drop policy if exists "sessions_select_all" on public.sessions;
create policy "sessions_select_all" on public.sessions
    for select to authenticated using (true);

-- Any authenticated user can create or update a session evening
drop policy if exists "sessions_insert_authenticated" on public.sessions;
create policy "sessions_insert_authenticated" on public.sessions
    for insert to authenticated
    with check (true);

drop policy if exists "sessions_update_creator_or_admin" on public.sessions;
create policy "sessions_update_creator_or_admin" on public.sessions
    for update to authenticated
    using (true)
    with check (true);

-- Only Admin can delete an entire session evening
drop policy if exists "sessions_delete_admin" on public.sessions;
create policy "sessions_delete_admin" on public.sessions
    for delete to authenticated
    using (public.is_current_user_admin());

-- Policies for `session_results`:
-- Only authenticated (logged-in) users can read results
drop policy if exists "results_select_all" on public.session_results;
create policy "results_select_all" on public.session_results
    for select to authenticated using (true);

-- Any authenticated user can insert or update money results for an evening
drop policy if exists "results_insert_own_or_admin" on public.session_results;
create policy "results_insert_own_or_admin" on public.session_results
    for insert to authenticated
    with check (true);

drop policy if exists "results_update_own_or_admin" on public.session_results;
create policy "results_update_own_or_admin" on public.session_results
    for update to authenticated
    using (true)
    with check (true);

drop policy if exists "results_delete_own_or_admin" on public.session_results;
create policy "results_delete_own_or_admin" on public.session_results
    for delete to authenticated
    using (true);

-- 5. Keep-alive RPC for GitHub Actions cron
create or replace function public.ping_keepalive()
returns text
language sql
security definer
set search_path = public
as $$
    select 'ok'::text;
$$;

revoke all on public.players, public.sessions, public.session_results from anon;
grant select, insert, update, delete on public.players, public.sessions, public.session_results to authenticated;
grant execute on function public.claim_player_profile(uuid) to authenticated;
grant execute on function public.is_current_user_admin() to authenticated;
grant execute on function public.current_user_player_id() to authenticated;
grant execute on function public.ping_keepalive() to anon, authenticated;

-- Force PostgREST to reload its schema cache immediately
notify pgrst, 'reload schema';


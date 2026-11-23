"""Same deals, both solvers: `compare.py GAME SOLVITAIRE_TYPE FIRST LAST REVERSE_STOCK TIMEOUT_S > out/GAME.txt`.
Set DEALBREAKER and SOLVITAIRE to the binaries. See README 'Against other solvers'."""
import json, subprocess, sys, time, re, os
D=os.environ.get("DEALBREAKER","target/release/dealbreaker")
SV=os.environ.get("SOLVITAIRE","solvitaire")
W=os.environ.get("DEALS_DIR","deals"); os.makedirs(W,exist_ok=True)
def card(c):
    c=c.lstrip("~"); return c.replace("T","10")
def convert(deal, stock_rev):
    p=deal["piles"]
    out={"tableau piles":[[card(c) for c in pile] for pile in p["tableau"]]}
    if "stock" in p:
        s=[card(c) for c in p["stock"][0]]
        out["stock"]=s[::-1] if stock_rev else s
    return out
def run_sv(typ,f,to):
    t=time.time()
    try:
        r=subprocess.run([SV,"--type",typ,f],capture_output=True,text=True,timeout=to)
    except subprocess.TimeoutExpired: return "timeout",to
    el=time.time()-t
    m=re.search(r"Solution Type: (\S+)",r.stdout)
    return (m.group(1) if m else "error:"+r.stderr[:80]),el
def run_db(game,f,to):
    t=time.time()
    try:
        r=subprocess.run([D,"solve",game,"--state",f,"--json","--max-states","200000000"],capture_output=True,text=True,timeout=to)
    except subprocess.TimeoutExpired: return "timeout",to
    el=time.time()-t
    o=json.loads(r.stdout)["outcome"]
    return {"solved":"solved","unsolvable":"unsolvable","gave-up":"timeout"}[o],el
if __name__=="__main__":
    game,typ,a,b,rev,to=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),sys.argv[5]=="1",float(sys.argv[6])
    for seed in range(a,b+1):
        d=json.loads(subprocess.run([D,"deal",game,"--seed",str(seed)],capture_output=True,text=True).stdout)
        f=f"{W}/{game}-{seed}.state.json"; g=f"{W}/{game}-{seed}.sv.json"
        json.dump(d,open(f,"w")); json.dump(convert(d,rev),open(g,"w"))
        print(seed,run_db(game,f,to),run_sv(typ,g,to),flush=True)

# Example Terraform project for memo-rs AWS resouces

## Usage

```
terraform init
terraform plan
terraform apply
terraform destroy
```

## IAM Access Key Rotation Runbook

This runbook covers zero-downtime rotation for the programmatic IAM user created by this Terraform project.

Current resources:
- IAM user: `aws_iam_user.app_user`
- IAM access key: `aws_iam_access_key.app_user`
- IAM role: `aws_iam_role.app_s3_role`

> Important: IAM only allows 2 active access keys per user.

---

## Preferred Method: Terraform-managed rotation

Use this when key lifecycle should stay fully aligned with Terraform state.

### 1) Capture current key references

From `memo-rs-aws/`:

```bash
terraform output -raw iam_access_key_id
terraform output -raw iam_secret_access_key
```

Store these securely if needed for rollback checks.

### 2) Rotate the Terraform key resource

Force replacement of the key resource:

```bash
terraform apply -replace="aws_iam_access_key.app_user[0]"
```

Terraform will create a new key and remove the old one in the same apply.

### 3) Capture new credentials immediately

After apply:

```bash
terraform output -raw iam_access_key_id
terraform output -raw iam_secret_access_key
```

Save in your secret manager immediately.

### 4) Update applications and verify

- Update all services/CI jobs that use this IAM user key.
- Verify app flows: upload, list, download, and delete.
- Confirm assume-role flow still works (`sts:AssumeRole` into `iam_role_arn`).

### 5) Post-rotation checks

```bash
terraform plan -input=false
```

Expected: no drift.

---

## Alternative: Console rotation (not recommended for Terraform-managed key)

You can rotate in AWS Console, but this introduces Terraform drift if `aws_iam_access_key.app_user` remains managed.

If you must rotate in console:
1. Create second key in console.
2. Update applications to use new key.
3. Disable old key, validate app.
4. Delete old key.
5. Reconcile Terraform state/config after rotation (or Terraform may try to recreate/replace key unexpectedly).

---

## Emergency rollback

If app fails after rotation:
1. Re-enable previous key (if still present and not deleted).
2. Repoint app secrets to previous key.
3. Investigate assumed role usage, region, and presigned URL generation.
4. Perform controlled re-rotation.

---

## Security notes

- `iam_secret_access_key` is marked sensitive in Terraform output but remains in Terraform state.
- Restrict access to backend state and IAM permissions.
- Do not paste keys into chat, tickets, or logs.
- Consider long-term migration from static keys to workload identity (role-based credentials) where possible.

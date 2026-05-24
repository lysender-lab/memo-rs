variable "aws_region" {
  description = "AWS region where resources are created"
  type        = string
  default     = "ap-southeast-1"
}

variable "s3_bucket_name" {
  description = "S3 bucket name for storing memo-rs files"
  type        = string
  default     = "memo-rs"
}

variable "aws_s3_policy_name" {
  description = "Policy name for S3 access permissions"
  type        = string
  default     = "memo-rs-s3-access"
}

variable "allowed_origins" {
  description = "Allowed browser origins for S3 CORS requests"
  type        = list(string)
  default     = ["http://localhost:11000", "https://memories-awesome-domain.com"]

  validation {
    condition     = length(var.allowed_origins) > 0
    error_message = "allowed_origins must contain at least one origin."
  }
}

variable "tags" {
  description = "Tags to apply to created resources"
  type        = map(string)
  default = {
    ManagedBy = "Terraform"
  }
}

variable "iam_username" {
  description = "IAM username for programmatic access"
  type        = string
  default     = "memo-rs-user"
}

variable "create_iam_user" {
  description = "Whether to create an IAM user for programmatic access"
  type        = bool
  default     = true
}

variable "create_iam_access_key" {
  description = "Whether to create a long-lived IAM access key for the IAM user"
  type        = bool
  default     = true

  validation {
    condition     = var.create_iam_access_key ? var.create_iam_user : true
    error_message = "create_iam_access_key can only be true when create_iam_user is true."
  }
}
